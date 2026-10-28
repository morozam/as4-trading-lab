#!/usr/bin/env python3
"""Export a Linux/WSL2 test kit without runtime databases, logs or local credentials."""
from pathlib import Path
import os, tarfile

ROOT=Path(__file__).resolve().parent
OUT=ROOT/'dist/nomination-lab-colleague-linux.tar.gz'

def main():
 if not (ROOT/'target/debug/nomination-lab').exists() or not (ROOT/'client/dist/index.html').exists():raise SystemExit('Build the application first: ./run.sh build')
 OUT.parent.mkdir(exist_ok=True)
 with tarfile.open(OUT,'w:gz',compresslevel=5) as archive:
  for name in ['Cargo.toml','Cargo.lock','CONTRACT.md','README.md','THIRD_PARTY.md','run.sh','run.py','setup.py','configure-peer.py']:
   archive.add(ROOT/name,arcname='nomination-lab-kit/nomination-lab/'+name)
  for name in ['docs','src','schemas','tests','gateway/assets','gateway/licenses','gateway/pmodes','client/dist','client/src','client/test']:
   archive.add(ROOT/name,arcname='nomination-lab-kit/nomination-lab/'+name,filter=lambda info:None if '__pycache__' in info.name else info)
  for name in ['package.json','package-lock.json','tsconfig.json','vite.config.ts','index.html']:
   archive.add(ROOT/'client'/name,arcname='nomination-lab-kit/nomination-lab/client/'+name)
  archive.add(ROOT/'gateway/LoopbackHTTPListener.java',arcname='nomination-lab-kit/nomination-lab/gateway/LoopbackHTTPListener.java')
  archive.add(ROOT/'gateway/RequirePayloadEncryption.java',arcname='nomination-lab-kit/nomination-lab/gateway/RequirePayloadEncryption.java')
  archive.add(ROOT/'gateway/INTEGRATION-CONTRACT.md',arcname='nomination-lab-kit/nomination-lab/gateway/INTEGRATION-CONTRACT.md')
  archive.add(ROOT/'target/debug/nomination-lab',arcname='nomination-lab-kit/nomination-lab/target/debug/nomination-lab')
  source=Path(os.environ.get('LAB_HOLODECK_SOURCE',str(ROOT.parent/'holodeckb2b-8.1.1'))).resolve()
  for name in ['bin','lib','conf','repository/xsd','repository/services','examples/certs','license.txt','bc_license.txt','README.md']:
   archive.add(source/name,arcname='nomination-lab-kit/holodeckb2b-8.1.1/'+name)
  java=Path(os.environ.get('LAB_JAVA_HOME',str(ROOT.parent/'java17'))).resolve()
  archive.add(java,arcname='nomination-lab-kit/java17')
 print(f'{OUT} ({OUT.stat().st_size/1024/1024:.1f} MiB)')
 print('Contains only public vendor example keys; no runtime data, sessions or company keys.')

if __name__=='__main__':main()
