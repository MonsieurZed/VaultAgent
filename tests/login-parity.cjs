/* Comparer la dérivation officielle et le moteur avec des identifiants entièrement factices. */
const https = require('node:https');
const { spawn, spawnSync } = require('node:child_process');
const { createInterface } = require('node:readline');
const { pbkdf2Sync } = require('node:crypto');
const { mkdtempSync, rmSync, readFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve, dirname, basename } = require('node:path');

const root = resolve(__dirname, '..');
const official = join(process.env.LOCALAPPDATA, 'Homelab/VaultCli/node_modules/@bitwarden/cli/build/bw.js');
const email = 'probe@example.invalid';
const password = 'Fictif-éè-"\\-日本語-42!';
const key = pbkdf2Sync(password, email, 600000, 32, 'sha256');
const expected = pbkdf2Sync(key, password, 1, 32, 'sha256').toString('base64');
const profiles = [];
const tlsDirectory=mkdtempSync(join(tmpdir(),'vault-parity-'));
profiles.push(tlsDirectory);
const certificate=spawnSync('pwsh.exe',['-NoProfile','-File',join(__dirname,'create-test-certificate.ps1'),'-Directory',tlsDirectory],{windowsHide:true});
if(certificate.status!==0)throw new Error('Test certificate generation failed');
const observations = [];
const paths = [];
let current;
const server = https.createServer({key:readFileSync(join(tlsDirectory,'test.key')),cert:readFileSync(join(tlsDirectory,'test.crt'))},async (request, response) => {
  paths.push({client:current,path:request.url.split('?')[0]});
  let body = ''; for await (const chunk of request) body += chunk;
  const base = `https://127.0.0.1:${server.address().port}`;
  response.setHeader('content-type', 'application/json');
  if (request.url.startsWith('/api/config')) {
    response.end(JSON.stringify({object:'config',version:'2026.6.0',gitHash:'',featureStates:{},
      environment:{vault:base,api:base+'/api',identity:base+'/identity',icons:base+'/icons',notifications:base+'/notifications',events:base+'/events'},
      server:{name:'Vaultwarden',url:base},push:{pushTechnology:0},settings:{},communication:{}}));
  } else if (request.url.includes('/accounts/prelogin')) {
    response.end(JSON.stringify({kdf:0,kdfIterations:600000,salt:email,kdfSettings:{kdfType:0,iterations:600000}}));
  } else if (request.url.startsWith('/identity/connect/token')) {
    const values = new URLSearchParams(body);
    observations.push({client:current,correctEmail:values.get('username')===email,correctHash:values.get('password')===expected});
    response.statusCode=400;
    response.end(JSON.stringify({error:'invalid_grant',error_description:'Username or password is incorrect. Try again.',ErrorModel:{Message:'Username or password is incorrect. Try again.'}}));
  } else { response.statusCode=404; response.end('{}'); }
});
function start(entry, args, profile) {
  const env = {...process.env};
  for (const name of Object.keys(env)) if (name.startsWith('BW_') || name.startsWith('NODE_')) delete env[name];
  Object.assign(env,{BITWARDENCLI_APPDATA_DIR:profile,BW_NOINTERACTION:'true',VAULT_PARITY_PASSWORD:password,NODE_EXTRA_CA_CERTS:join(tlsDirectory,'test.crt')});
  return spawn(join(root,'runtime/node.exe'),[entry,...args],{env,windowsHide:true,stdio:['pipe','pipe','pipe']});
}
function runOfficial(args, profile) {
  return new Promise((resolve,reject)=>{
    const child=start(official,args,profile); child.stdout.resume();child.stderr.resume();
    const timeout=setTimeout(()=>{child.kill();reject(new Error('Official CLI timeout'));},30000);
    child.on('error',reject);child.on('close',code=>{clearTimeout(timeout);resolve(code);});
  });
}
async function main() {
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const base=`https://127.0.0.1:${server.address().port}`;
  const profile=()=>{const value=mkdtempSync(join(tmpdir(),'vault-parity-'));profiles.push(value);return value;};
  current='official';const officialProfile=profile();
  if(await runOfficial(['config','server',base],officialProfile)!==0)throw new Error('Official config failed');
  await runOfficial(['login',email,'--passwordenv','VAULT_PARITY_PASSWORD'],officialProfile);
  current='persistent';
  const persistentProfile=profile();
  async function invokeEngine(op) { return new Promise((resolve,reject)=>{
    const child=start(join(root,'runtime/engine/bw.js'),[],persistentProfile);child.stderr.resume();
    const timeout=setTimeout(()=>{child.kill();reject(new Error('Persistent engine timeout'));},30000);
    createInterface({input:child.stdout}).on('line',line=>{
      const result=JSON.parse(line);
      if(result.ready)child.stdin.write(JSON.stringify({id:1,op,email,...(op==='connect'?{password}:{}),server:base})+'\n');
      else{child.kill();clearTimeout(timeout);child.once('close',()=>resolve(result));}
    });child.on('error',reject);
  }); }
  const prepared=await invokeEngine('prepare');
  if(!prepared.success||!prepared.data.restartRequired)throw new Error('Initial server preparation failed');
  await invokeEngine('connect');
  const success=observations.length===2&&observations.every(value=>value.correctEmail&&value.correctHash);
  console.log(JSON.stringify({success,observations,paths,onlyFictitiousCredentials:true}));
  if(!success)process.exitCode=1;
}
main().catch(error=>{console.error(error.message);process.exitCode=1;}).finally(()=>{
  server.close();for(const profile of profiles){
    const target=resolve(profile);
    if(target===profile&&dirname(target)===resolve(tmpdir())&&basename(target).startsWith('vault-parity-'))try{rmSync(target,{recursive:true,force:true});}catch{}
  }
});
