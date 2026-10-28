#!/usr/bin/env python3
"""Prepare isolated, reproducible laboratory nodes; existing gateways are read-only."""
from pathlib import Path
import copy, hashlib, json, os, shutil, subprocess, sys, uuid
import xml.etree.ElementTree as ET

ROOT=Path(__file__).resolve().parent
SOURCE=Path(os.environ.get('LAB_HOLODECK_SOURCE',str(ROOT.parent/'holodeckb2b-8.1.1'))).resolve()
JAVA=Path(os.environ.get('LAB_JAVA_HOME',str(ROOT.parent/'java17'))).resolve()
ASSETS={
 'rest-backend-4.0.0.jar':'808795325a56e20f7aedda9057c5e2e9ad7c787afcab5bbaf71147fab86325bc',
 'hb2b-rest-backend.aar':'369c7bcb5063900da6c0a4cb4347cc3d0b12d95408a7e90ddc8e095b117fe394',
 'rest-empty-error-2.0.0.mar':'7e9f1ca6c4a24cc44573a405892b6ee165c7c19578960e42dc4f9a9587f374c3',
}
PNS='http://holodeck-b2b.org/schemas/2014/10/pmode'
WNS='http://holodeck-b2b.org/schemas/2012/12/workers'
SECURITY_CLASS='lab.holodeck.RequirePayloadEncryption'

def secure_pmode(root):
 # The receiver owns the decryption key. Its existing signing alias identifies
 # the same lab certificate; preserve whether this node has private-key access.
 security=root.find(f'{{{PNS}}}Responder/{{{PNS}}}SecurityConfiguration')
 if security is None:raise ValueError('Lab P-Mode has no receiver security configuration')
 alias=security.find(f'{{{PNS}}}Signing/{{{PNS}}}KeystoreAlias')
 if alias is None:raise ValueError('Lab P-Mode has no receiver certificate alias')
 for old in security.findall(f'{{{PNS}}}Encryption'):security.remove(old)
 encryption=ET.SubElement(security,f'{{{PNS}}}Encryption')
 encryption.append(copy.deepcopy(alias))
 ET.SubElement(encryption,f'{{{PNS}}}Algorithm').text='http://www.w3.org/2009/xmlenc11#aes128-gcm'
 transport=ET.SubElement(encryption,f'{{{PNS}}}KeyTransport')
 for name,value in [('Algorithm','http://www.w3.org/2009/xmlenc11#rsa-oaep'),('MGFAlgorithm','http://www.w3.org/2009/xmlenc11#mgf1sha256'),('DigestAlgorithm','http://www.w3.org/2001/04/xmlenc#sha256'),('KeyReferenceMethod','IssuerSerial')]:
  ET.SubElement(transport,f'{{{PNS}}}{name}').text=value
 flow=root.find(f'{{{PNS}}}Leg/{{{PNS}}}UserMessageFlow')
 profile=flow.find(f'{{{PNS}}}PayloadProfile')
 if profile is None:
  profile=ET.Element(f'{{{PNS}}}PayloadProfile')
  position=next((i for i,c in enumerate(flow) if c.tag==f'{{{PNS}}}CustomValidation'),len(flow))
  flow.insert(position,profile)
 for old in profile.findall(f'{{{PNS}}}UseAS4Compression'):profile.remove(old)
 ET.SubElement(profile,f'{{{PNS}}}UseAS4Compression').text='true'

def save_security_xml(path,tree,gateway):
 data=ET.tostring(tree.getroot(),encoding='UTF-8',xml_declaration=True)
 if path.read_bytes()==data:return
 backup=gateway.parent/'security-backup'/path.relative_to(gateway)
 backup.parent.mkdir(parents=True,exist_ok=True)
 if not backup.exists():shutil.copy2(path,backup)
 path.write_bytes(data)

def install_security(gateway):
 shutil.copy2(ROOT/'gateway/assets/lab-loopback-transport.jar',gateway/'lib')
 # Upgrade only known lab routes, retaining endpoint/tunnel and callback settings.
 for name in ['node-a-forward-sender.xml','node-a-reverse-receiver.xml','node-b-forward-receiver.xml','node-b-reverse-sender.xml']:
  path=gateway/'repository/pmodes'/name
  if path.exists():
   tree=ET.parse(path);secure_pmode(tree.getroot());save_security_xml(path,tree,gateway)
 conf=gateway/'conf/holodeckb2b.xml';tree=ET.parse(conf)
 for flow in tree.getroot().findall('phaseOrder'):
  if flow.get('type') not in ['InFlow','InFaultFlow']:continue
  phase=flow.find('phase[@name="LabSecurityPolicy"]')
  if phase is None:
   phase=ET.Element('phase',{'name':'LabSecurityPolicy'})
   index=next(i for i,p in enumerate(flow) if p.get('name')=='Security')+1
   flow.insert(index,phase)
  for existing in list(phase):phase.remove(existing)
  ET.SubElement(phase,'handler',{'name':'RequirePayloadEncryption','class':SECURITY_CLASS})
 save_security_xml(conf,tree,gateway)

def main():
 if not (SOURCE/'lib').is_dir():raise SystemExit('Set LAB_HOLODECK_SOURCE to an unpacked Holodeck 8.1.1 distribution.')
 if not (JAVA/'bin/java').exists():raise SystemExit('Set LAB_JAVA_HOME to Java 17.')
 for name,digest in ASSETS.items():
  path=ROOT/'gateway/assets'/name
  if hashlib.sha256(path.read_bytes()).hexdigest()!=digest:raise SystemExit(f'Gateway artifact checksum mismatch: {name}')
 build=ROOT/'gateway/build';build.mkdir(exist_ok=True)
 jar=ROOT/'gateway/assets/lab-loopback-transport.jar'
 sources=[ROOT/'gateway'/name for name in ['LoopbackHTTPListener.java','RequirePayloadEncryption.java']]
 if not jar.exists() or any(source.stat().st_mtime>jar.stat().st_mtime for source in sources):
  subprocess.run(['javac','--release','17','-cp',str(SOURCE/'lib/*'),'-d',str(build),*[str(p) for p in sources]],check=True)
  subprocess.run(['jar','cf',str(jar),'-C',str(build),'lab'],check=True)
 for role,app_port,gateway_port in [('brp',8797,19080),('tso',8798,19081)]:
  node=ROOT/'runtime'/role;node.mkdir(parents=True,exist_ok=True)
  config_path=node/'config.json'
  if config_path.exists():config=json.loads(config_path.read_text())
  else:
   config={'role':role,'bind':'127.0.0.1','port':app_port,'data_dir':str(node/'app'),'schema_dir':str(ROOT/'schemas'),'ui_dir':str(ROOT/'client/dist'),
      'gateway_url':f'http://127.0.0.1:{gateway_port}/holodeckb2b/restbackend/submit','peer_url':f'http://127.0.0.1:{8798 if role=="brp" else 8797}',
      'pmode_id':'mvp-forward' if role=='brp' else 'mvp-reverse','inbound_pmode_id':'mvp-reverse' if role=='brp' else 'mvp-forward',
      'callback_token':uuid.uuid4().hex+uuid.uuid4().hex,'username':role,'password':f'local-{role}'}
   config_path.write_text(json.dumps(config,indent=2)+'\n');config_path.chmod(0o600)
  gateway=node/'gateway'
  if (gateway/'conf/holodeckb2b.xml').exists():
   install_security(gateway)
   print(f'{role.upper()}: isolated gateway retained; encryption/compression policy installed')
   continue
  gateway.mkdir(exist_ok=True)
  for folder in ['bin','lib','conf']:
   shutil.copytree(SOURCE/folder,gateway/folder,dirs_exist_ok=True)
  for folder in ['repository/xsd','repository/services']:
   shutil.copytree(SOURCE/folder,gateway/folder,dirs_exist_ok=True)
  for folder in ['repository/certs','repository/pmodes','repository/modules','db','data/msg_in','data/msg_out','temp','logs']:
   (gateway/folder).mkdir(parents=True,exist_ok=True)
  for cert in (SOURCE/'examples/certs').glob('*.jks'):shutil.copy2(cert,gateway/'repository/certs'/cert.name)
  shutil.copy2(ROOT/'gateway/assets/rest-backend-4.0.0.jar',gateway/'lib')
  shutil.copy2(jar,gateway/'lib')
  shutil.copy2(ROOT/'gateway/assets/hb2b-rest-backend.aar',gateway/'repository/services')
  shutil.copy2(ROOT/'gateway/assets/rest-empty-error-2.0.0.mar',gateway/'repository/modules')
  conf=gateway/'conf/holodeckb2b.xml';tree=ET.parse(conf)
  receiver=tree.getroot().find('transportReceiver')
  receiver.set('class','lab.holodeck.LoopbackHTTPListener')
  for p in receiver.findall('parameter'):
   if p.get('name')=='port':p.text=str(gateway_port)
  tree.write(conf,encoding='UTF-8',xml_declaration=True)
  workers=gateway/'conf/workers.xml';tree=ET.parse(workers)
  for worker in tree.getroot().findall(f'{{{WNS}}}worker'):
   if worker.get('name') in ['monitorAPI','submitFromFileWorker']:worker.set('activate','false')
   if worker.get('name')=='senderWorker':worker.set('interval','1');worker.set('delay','1')
  tree.write(workers,encoding='UTF-8',xml_declaration=True)
  # All credentials here are public vendor example keys. Never copy local repository/certs.
  certconf=gateway/'conf/certmanager_config.xml';tree=ET.parse(certconf);ns={'c':'http://holodeck-b2b.org/schemas/2019/09/config/certmanager'}
  for name,password in [('PrivateKeys','secrets'),('TradingPartnerCertificates','nosecrets'),('TrustedCertificates','trusted')]:
   tree.find(f'.//c:{name}/c:Password',ns).text=password
  tree.write(certconf,encoding='UTF-8',xml_declaration=True)
  names=['node-a-forward-sender.xml','node-a-reverse-receiver.xml'] if role=='brp' else ['node-b-forward-receiver.xml','node-b-reverse-sender.xml']
  for name in names:
   tree=ET.parse(ROOT/'gateway/pmodes'/name)
   for param in tree.getroot().findall(f'.//{{{PNS}}}DefaultDelivery/{{{PNS}}}Parameter'):
    if param.find(f'{{{PNS}}}name').text=='URL':param.find(f'{{{PNS}}}value').text=f'http://127.0.0.1:{app_port}/integrations/{config["callback_token"]}'
   tree.write(gateway/'repository/pmodes'/name,encoding='UTF-8',xml_declaration=True)
  install_security(gateway)
  (node/'logs').mkdir(exist_ok=True)
  print(f'{role.upper()}: clean AS4 gateway prepared on loopback:{gateway_port}')
 print('Local configuration ready. Both nodes have independent databases and gateway state.')

if __name__=='__main__':main()
