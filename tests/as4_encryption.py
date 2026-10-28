#!/usr/bin/env python3
"""Capture real loopback AS4 requests and prove encryption/decryption and rejection.

Temporary outbound P-Modes use a loopback recording proxy. The normal inbound
routes, keys, databases and application callbacks are used without alteration.
Only hashes and public security metadata are saved as test evidence.
"""
from pathlib import Path
from email.parser import BytesParser
from email import policy
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import Request, urlopen
from urllib.error import HTTPError
import hashlib, http.client, json, subprocess, threading, time, uuid
import xml.etree.ElementTree as ET
from http_roundtrip import Client, wait

ROOT = Path(__file__).resolve().parents[1]
PNS = 'http://holodeck-b2b.org/schemas/2014/10/pmode'
XENC = 'http://www.w3.org/2001/04/xmlenc#'
EB = 'http://docs.oasis-open.org/ebxml-msg/ebms/v3.0/ns/core/200704/'
DS = 'http://www.w3.org/2000/09/xmldsig#'
AES = 'http://www.w3.org/2009/xmlenc11#aes128-gcm'
RSA = 'http://www.w3.org/2009/xmlenc11#rsa-oaep'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_body(handler):
    if handler.headers.get('Transfer-Encoding', '').lower() == 'chunked':
        chunks = []
        while True:
            size = int(handler.rfile.readline().split(b';')[0], 16)
            if size == 0:
                while handler.rfile.readline().strip():
                    pass
                return b''.join(chunks)
            chunks.append(handler.rfile.read(size))
            assert handler.rfile.read(2) == b'\r\n'
    return handler.rfile.read(int(handler.headers['Content-Length']))


class CaptureProxy(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_):
        pass

    def do_POST(self):
        body = read_body(self)
        mime = BytesParser(policy=policy.default).parsebytes(
            b'Content-Type: ' + self.headers['Content-Type'].encode() + b'\r\nMIME-Version: 1.0\r\n\r\n' + body)
        parts = list(mime.iter_parts())
        soap = ET.fromstring(parts[0].get_payload(decode=True))
        attachment = parts[1].get_payload(decode=True)
        record = {
            'message_id': soap.findtext(f'.//{{{EB}}}MessageId'),
            'signed': soap.find(f'.//{{{DS}}}Signature') is not None,
            'encrypted_data': [n.get('Algorithm') for n in soap.findall(f'.//{{{XENC}}}EncryptedData/{{{XENC}}}EncryptionMethod')],
            'key_transport': [n.get('Algorithm') for n in soap.findall(f'.//{{{XENC}}}EncryptedKey/{{{XENC}}}EncryptionMethod')],
            'compression': [p.get('name') + '=' + (p.text or '') for p in soap.findall(f'.//{{{EB}}}PartProperties/{{{EB}}}Property')],
            'wire_attachment_bytes': len(attachment),
            'wire_attachment_sha256': digest(attachment),
            'raw_xml_visible': self.server.expected_xml in body,
        }
        if self.server.tamper:
            offset = body.index(attachment) + len(attachment) // 2
            body = body[:offset] + bytes([body[offset] ^ 1]) + body[offset + 1:]
            record['ciphertext_modified'] = True
        skip = {'host', 'content-length', 'transfer-encoding', 'connection', 'keep-alive', 'te', 'trailer', 'upgrade'}
        headers = {k: v for k, v in self.headers.items() if k.lower() not in skip}
        headers['Connection'] = 'close'
        conn = http.client.HTTPConnection('127.0.0.1', self.server.destination, timeout=20)
        try:
            conn.request('POST', self.path, body=body, headers=headers)
            response = conn.getresponse()
            payload = response.read()
            reply = ET.fromstring(payload)
            record['response_errors'] = [e.get('errorCode') for e in reply.findall(f'.//{{{EB}}}Error')]
            record['response_receipt'] = reply.find(f'.//{{{EB}}}Receipt') is not None
            self.server.records.append(record)
            self.send_response(response.status)
            for name, value in response.getheaders():
                if name.lower() not in skip | {'server', 'date'}:
                    self.send_header(name, value)
            self.send_header('Content-Length', str(len(payload)))
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.write(payload)
            self.close_connection = True
        finally:
            conn.close()


def submit(port, mode, message_id, xml):
    deadline = time.monotonic() + 30
    while True:
        request = Request(f'http://127.0.0.1:{port}/holodeckb2b/restbackend/submit', data=xml,
                          headers={'Content-Type': 'application/xml',
                                   'X-HolodeckB2B-PModeId': mode,
                                   'X-HolodeckB2B-MessageId': message_id,
                                   'X-HolodeckB2B-ConversationId': 'security-' + uuid.uuid4().hex,
                                   'X-HolodeckB2B-Containment': 'ATTACHMENT'})
        try:
            with urlopen(request, timeout=10) as response:
                assert response.status == 202
                return
        except HTTPError as error:
            detail = error.headers.get('X-Error', '').lower()
            if error.code != 400 or 'mode' not in detail or time.monotonic() > deadline:
                raise AssertionError(f'Probe submission rejected: HTTP {error.code}; {detail}') from error
            time.sleep(.3)  # P-Mode watcher has not loaded the new test route yet.


def probe(direction, negative=None):
    forward = direction == 'brp-to-tso'
    source, target = ('brp', 'tso') if forward else ('tso', 'brp')
    source_port, target_port = (19080, 19081) if forward else (19081, 19080)
    target_client = Client(target, 8798 if forward else 8797)
    fixture = ROOT / 'tests/fixtures/reference' / ('nomint_multi.xml' if forward else 'nomres_exchange.xml')
    business = ET.fromstring(fixture.read_bytes())
    business[0].text = 'SEC' + uuid.uuid4().hex
    xml = ET.tostring(business, encoding='utf-8', xml_declaration=True)
    proxy = ThreadingHTTPServer(('127.0.0.1', 0), CaptureProxy)
    proxy.destination = target_port
    proxy.expected_xml = xml
    proxy.tamper = negative == 'tampered'
    proxy.records = []
    worker = threading.Thread(target=proxy.serve_forever, daemon=True)
    worker.start()
    mode_id = 'encryption-probe-' + uuid.uuid4().hex
    directory = ROOT / 'runtime' / source / 'gateway/repository/pmodes'
    template = directory / ('node-a-forward-sender.xml' if forward else 'node-b-reverse-sender.xml')
    tree = ET.parse(template)
    tree.getroot().find(f'{{{PNS}}}id').text = mode_id
    tree.getroot().find(f'{{{PNS}}}Leg/{{{PNS}}}Protocol/{{{PNS}}}Address').text = f'http://127.0.0.1:{proxy.server_port}/holodeckb2b/as4'
    if negative == 'plaintext':
        security = tree.getroot().find(f'{{{PNS}}}Responder/{{{PNS}}}SecurityConfiguration')
        security.remove(security.find(f'{{{PNS}}}Encryption'))
    path = directory / (mode_id + '.xml')
    message_id = uuid.uuid4().hex + '@encryption-probe'
    try:
        path.touch(mode=0o600)
        tree.write(path, encoding='UTF-8', xml_declaration=True)
        subprocess.run(['xmllint', '--nonet', '--noout', '--schema', str(ROOT/'runtime'/source/'gateway/repository/xsd/pmode.xsd'), str(path)], check=True, capture_output=True)
        submit(source_port, mode_id, message_id, xml)
        record = wait(lambda: next((r for r in proxy.records if r['message_id'] == message_id), None), 'AS4 wire capture', timeout=40)
        assert record['signed'], record
        assert 'CompressionType=application/gzip' in record['compression'], record
        if negative == 'plaintext':
            assert not record['encrypted_data'], record
            assert 'EBMS:0103' in record['response_errors'], record
        else:
            assert record['encrypted_data'] and set(record['encrypted_data']) == {AES}, record
            assert record['key_transport'] == [RSA], record
            assert not record['raw_xml_visible'], record
            assert record['wire_attachment_sha256'] != digest(xml), record
        if negative:
            assert record['response_errors'] and not record['response_receipt'], record
            assert not any(m['id'] == message_id for m in target_client.state()['messages']), 'Rejected bytes reached the business application'
        else:
            assert record['response_receipt'] and not record['response_errors'], record
            received = wait(lambda: next((m for m in target_client.state()['messages'] if m['id'] == message_id), None), 'decrypted business callback')
            assert received['xml'].encode() == xml, 'Decrypted payload differs from submitted bytes'
            assert received['hash'] == digest(xml)
            record['decrypted_xml_sha256'] = received['hash']
            record['exact_bytes_preserved'] = True
        record.update(direction=direction, test=negative or 'encrypted delivery')
        print('PASS:', direction, negative or 'encrypted + signed + compressed, exact decrypted XML')
        return record
    finally:
        path.unlink(missing_ok=True)
        proxy.shutdown()
        proxy.server_close()
        worker.join(timeout=3)


def main():
    evidence = []
    for direction in ['brp-to-tso', 'tso-to-brp']:
        for negative in [None, 'plaintext', 'tampered']:
            evidence.append(probe(direction, negative))
    output = ROOT / 'test-output/as4-encryption.json'
    output.parent.mkdir(exist_ok=True)
    output.write_text(json.dumps(evidence, indent=2))
    print('PASS: both directions encrypt/compress/sign; plaintext and tampered ciphertext rejected before business delivery.')
    print(output)


if __name__ == '__main__':
    main()
