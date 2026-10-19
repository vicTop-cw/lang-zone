const {spawn}=require('child_process');
const c=spawn(process.execPath,['E:/IDEProjects/AI/FIST-Mbt/_build/js/debug/build/cmd/main/main.js'],{stdio:['pipe','pipe','pipe']});
let b='';let done=false;
c.stdout.on('data',d=>{b+=d;if(b.includes('\n')&&!done){done=true;const j=JSON.parse(b.split('\n')[0]);const names=(j.result.tools||[]).map(t=>t.name);process.stdout.write(names.join(' ')+'\n\n');(j.result.tools||[]).forEach(t=>{if(/omega|bug|standard|output|call_log|laya|issue/.test(t.name))process.stdout.write(t.name+' :: '+JSON.stringify(t.inputSchema&&t.inputSchema.properties?Object.keys(t.inputSchema.properties):null)+'\n')});c.kill();process.exit(0)}});
c.stdin.write(JSON.stringify({jsonrpc:'2.0',id:'1',method:'tools/list',params:{_meta:{'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientCapabilities':{},'io.modelcontextprotocol/clientInfo':{name:'d',version:'1'}}}})+'\n');
setTimeout(()=>{if(!done){process.stdout.write('TIMEOUT\n'+b+'\n');c.kill();process.exit(0)}},20000);
