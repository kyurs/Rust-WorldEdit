// Explicitly authorized test server only; WorldEditAdminQA needs temporary operator access.
const mc=require('minecraft-protocol'), assert=require('node:assert/strict');
const {setTimeout:delay}=require('node:timers/promises');
const host=process.env.WORLDEDIT_TEST_HOST;if(!host)throw Error('Set WORLDEDIT_TEST_HOST');
const clients=[];let fixture;
const plain=x=>typeof x==='string'?x:Array.isArray(x)?x.map(plain).join(''):(x?.text||x?.translate||'')+(x?.with||[]).map(plain).join('')+(x?.extra||[]).map(plain).join('');
async function wait(f,label){for(let i=0;i<250;i++){if(f())return;await delay(100);}throw Error('Timed out: '+label);}
async function join(name){const c=mc.createClient({host,port:25565,username:name,version:'1.12.2',auth:'offline'});const p={c,chat:[],packets:[],pos:null,items:[]};clients.push(p);c.on('error',e=>p.error=e);c.on('end',r=>p.error=Error(r));c.on('packet',(d,m)=>{p.packets.push({name:m.name,data:d});if(m.name==='position'){p.pos=d;c.write('teleport_confirm',{teleportId:d.teleportId});}if(m.name==='chat')p.chat.push(plain(JSON.parse(d.message)));if(m.name==='set_slot')p.items[d.slot]=d.item;if(m.name==='window_items'&&d.windowId===0)p.items=d.items;});c.on('login',()=>c.write('settings',{locale:'en_US',viewDistance:2,chatFlags:0,chatColors:true,skinParts:127,mainHand:1}));await wait(()=>p.pos||p.error,'join '+name);if(p.error)throw p.error;return p;}
async function cmd(p,message,expect){p.chat=[];p.c.write('chat',{message});if(expect)await wait(()=>p.chat.some(x=>x.includes(expect))||p.error,message+': '+expect);else await delay(500);if(p.error)throw p.error;console.log(message, p.chat);}
async function verify(p,x,y,z,block){await cmd(p,`/execute if block ${x} ${y} ${z} minecraft:${block} run say WE_CHECK_PASS`,'WE_CHECK_PASS');}
(async()=>{let p;try{
 const guest=await join('WEGuestQA');await cmd(guest,'//wand');assert(!guest.chat.some(x=>x.includes('Wooden axe added')));assert(!guest.items.some(x=>x?.blockId===271));console.log('PASS: non-operator denied wand');guest.c.end();guest.c.socket?.destroy();await delay(3500);
 p=await join('WorldEditAdminQA');await delay(1000);await cmd(p,'/gamemode creative');await cmd(p,'//wand','Wooden axe added');assert(p.items.some(x=>x?.blockId===271),'wooden axe delivered');
 const x=Math.floor(p.pos.x)+4,z=Math.floor(p.pos.z),y=240;
 for(const dx of [0,1,4,5])await verify(p,x+dx,y,z,'air');fixture={x,y,z};
 for(const dx of [0,1])await cmd(p,`/setblock ${x+dx} ${y} ${z} minecraft:stone`);
 await cmd(p,`/tp @s ${x+0.5} ${y+1} ${z+1.5}`);
 p.c.write('set_creative_slot',{slot:36,item:{blockId:271,itemCount:1,itemDamage:0,nbtData:undefined}});p.c.write('held_item_slot',{slotId:0});await delay(500);
 p.chat=[];p.c.write('block_dig',{status:0,location:{x,y,z},face:1});await wait(()=>p.chat.some(t=>t.includes('Position 1 set')),'left-click selection');
 p.chat=[];p.c.write('block_place',{location:{x:x+1,y,z},direction:1,hand:0,cursorX:0.5,cursorY:0.5,cursorZ:0.5});await wait(()=>p.chat.some(t=>t.includes('Position 2 set')),'right-click selection');
 await verify(p,x,y,z,'stone');await verify(p,x+1,y,z,'stone');console.log('PASS: wand selects without breaking blocks');
 await cmd(p,'//size','Selection: 2');await cmd(p,'//set gold_block','Edit complete.');for(const dx of [0,1])await verify(p,x+dx,y,z,'gold_block');
 await cmd(p,'//undo','Edit complete.');for(const dx of [0,1])await verify(p,x+dx,y,z,'stone');
 await cmd(p,'//redo','Edit complete.');await verify(p,x,y,z,'gold_block');
 await cmd(p,'//replace gold_block diamond_block','Edit complete.');await verify(p,x,y,z,'diamond_block');
 await cmd(p,'//copy','Copied 2');await cmd(p,`/tp @s ${x+4.5} ${y+1} ${z+1.5}`);await cmd(p,'//paste','Edit complete.');for(const dx of [4,5])await verify(p,x+dx,y,z,'diamond_block');await cmd(p,'//undo','Edit complete.');for(const dx of [4,5])await verify(p,x+dx,y,z,'air');console.log('PASS: set, replace, copy/paste, undo and redo');
 await cmd(p,`//pos1 ${x},${y},${z}`);await cmd(p,`//pos2 ${x+4096},${y},${z}`,'Position 2');await cmd(p,'//set stone','limit is 4096');
 await cmd(p,`//pos2 ${x},${y},${z}`,'Position 2');await cmd(p,`/setblock ${x} ${y} ${z} minecraft:chest`);await cmd(p,'//set stone','stored data');await verify(p,x,y,z,'chest');console.log('PASS: oversized edits and container data loss prevented');
 }finally{if(p&&fixture){for(const dx of [0,1,4,5])await cmd(p,`/setblock ${fixture.x+dx} ${fixture.y} ${fixture.z} minecraft:air`);await cmd(p,'/gamemode survival');}for(const p of clients){p.c.end();p.c.socket?.destroy();}}
})().then(()=>process.exit(0)).catch(e=>{console.error(e);process.exit(1);});
