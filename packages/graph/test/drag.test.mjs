import test from 'node:test';
import assert from 'node:assert/strict';
import {mountGraph} from '../src/index.js';

/** Minimal DOM double: enough of canvas/window for mountGraph's pointer paths. */
function harness(){
  const listeners=new Map();
  const ctx=new Proxy({},{get:(t,k)=>k in t?t[k]:()=>{},set:(t,k,v)=>{t[k]=v;return true;}});
  const style={};
  const canvas={style,tabIndex:0,width:0,height:0,setAttribute(){},getContext:()=>ctx,remove(){},title:'',
    addEventListener:(t,f)=>listeners.set(t,f),removeEventListener:(t)=>listeners.delete(t),
    getBoundingClientRect:()=>({left:0,top:0,width:400,height:400}),
    setPointerCapture(){},hasPointerCapture:()=>true,releasePointerCapture(){}};
  const win={devicePixelRatio:1,requestAnimationFrame:()=>1,cancelAnimationFrame(){},getComputedStyle:()=>({getPropertyValue:()=>''}),
    ResizeObserver:class{observe(){}disconnect(){}}};
  const doc={defaultView:win,createElement:(tag)=>tag==='canvas'?canvas:{style:{},setAttribute(){},remove(){},textContent:''}};
  const container={ownerDocument:doc,style:{},append(){},getBoundingClientRect:()=>({width:400,height:400})};
  const fire=(type,e={})=>listeners.get(type)?.({button:0,pointerId:1,preventDefault(){},...e});
  return {container,fire};
}
// Two nodes 100 units apart; fit() centres them, so A sits left of the canvas centre.
const graph={nodes:[{id:'A',label:'A',x:-50,y:0},{id:'B',label:'B',x:50,y:0}],edges:[{source:'A',target:'B'}]};
function mounted(){
  const h=harness();const calls=[];
  const record=(name)=>(node,p)=>calls.push([name,node?.id??null,p?{x:Math.round(p.x),y:Math.round(p.y)}:undefined]);
  const view=mountGraph(h.container,graph,{onSelect:record('select'),onActivate:record('activate'),onDragStart:record('start'),onDrag:record('drag'),onDragEnd:record('end')});
  const z=view.getState().zoom;const at=(x)=>({clientX:200+x*z,clientY:200});
  return {...h,view,calls,at,z};
}

test('press, move and release on a node drags it and does not select',()=>{
  const {fire,calls,at,view}=mounted();
  fire('pointerdown',at(-50));fire('pointermove',{...at(-50),clientX:at(-50).clientX+1});
  assert.deepEqual(calls,[],'sub-threshold movement must not start a drag');
  fire('pointermove',{...at(-20)});fire('pointerup',at(-20));
  assert.deepEqual(calls.map(c=>c[0]),['start','drag','end']);
  assert.ok(calls.every(c=>c[1]==='A'));
  assert.deepEqual(calls.at(-1)[2],{x:-20,y:0},'node follows the pointer');
  assert.equal(view.getState().selected,null);
});

test('grabbing off-centre keeps the pointer offset, so the node does not jump',()=>{
  const {fire,calls,at,z}=mounted();
  fire('pointerdown',{...at(-50),clientX:at(-50).clientX+4});fire('pointermove',{...at(-10),clientX:at(-10).clientX+4});
  assert.deepEqual(calls.find(c=>c[0]==='drag')[2],{x:-10,y:0});
  assert.ok(z>0);
});

test('press and release without movement selects and never drags',()=>{
  const {fire,calls,at,view}=mounted();
  fire('pointerdown',at(50));fire('pointerup',at(50));
  assert.deepEqual(calls,[['select','B',undefined]]);
  assert.equal(view.getState().selected,'B');
});

test('double-click still activates',()=>{
  const {fire,calls,at}=mounted();
  fire('pointerdown',at(50));fire('pointerup',at(50));fire('dblclick',at(50));
  assert.deepEqual(calls.map(c=>c[0]),['select','activate']);
});

test('background drag pans and emits no drag events',()=>{
  const {fire,calls,at}=mounted();
  fire('pointerdown',at(0));fire('pointermove',at(60));fire('pointerup',at(60));
  assert.deepEqual(calls,[]);
});

test('pointercancel, setGraph and destroy each end an active drag exactly once',()=>{
  for(const finish of [(m)=>m.fire('pointercancel'),(m)=>m.view.setGraph(graph),(m)=>m.view.destroy()]){
    const m=mounted();
    m.fire('pointerdown',m.at(-50));m.fire('pointermove',m.at(-30));finish(m);m.fire('pointerup',m.at(-30));
    assert.deepEqual(m.calls.map(c=>c[0]),['start','drag','end']);
  }
});

test('host position updates do not pull a node out from under the pointer',()=>{
  const {fire,calls,at,view}=mounted();
  fire('pointerdown',at(-50));fire('pointermove',at(-30));
  view.setPositions(new Float32Array([-90,0,90,0]));
  fire('pointerup',at(-30));
  assert.deepEqual(calls.at(-1),['end','A',{x:-30,y:0}]);
});
