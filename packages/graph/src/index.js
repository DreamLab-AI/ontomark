/** Shared graph view. Host applications supply data and navigation; this module
 * owns validation, traversal, rendering, pan/zoom and selection, with no I/O.
 * Radius scaling and orthographic interaction follow Narrative Goldmine's
 * GraphCanvas/palette design; the DOM adapter avoids a site-wide React store.
 */
export function validateGraph(graph) {
  const ids=new Set();
  for(const n of graph.nodes) {
    if(typeof n.id!=='string'||!n.id||ids.has(n.id)) throw new Error('Graph node IDs must be unique non-empty strings');
    if(typeof n.label!=='string') throw new Error('Graph node labels must be strings');
    if(n.degree!==undefined && (!Number.isFinite(n.degree)||n.degree<0)) throw new Error('Graph degree must be finite and non-negative');
    if((n.x!==undefined&&!Number.isFinite(n.x))||(n.y!==undefined&&!Number.isFinite(n.y))) throw new Error('Graph positions must be finite');
    ids.add(n.id);
  }
  for(const e of graph.edges) if(!ids.has(e.source)||!ids.has(e.target)) throw new Error('Graph edge references an absent node');
}
export function seedPositions(graph) {
  validateGraph(graph);
  const positions=new Map();
  graph.nodes.forEach((n,i)=>{
    // Golden-angle spacing avoids a single overlapping circle in large vaults.
    const angle=i*Math.PI*(3-Math.sqrt(5));const radius=24*Math.sqrt(i);
    positions.set(n.id,{x:n.x??Math.cos(angle)*radius,y:n.y??Math.sin(angle)*radius});
  });
  return positions;
}
export function neighbourhood(graph,centre,radius=1) {
  validateGraph(graph);
  if(!Number.isInteger(radius)||radius<0||radius>8) throw new Error('Graph radius must be an integer from 0 to 8');
  if(!graph.nodes.some(n=>n.id===centre)) throw new Error('Unknown graph centre');
  const adjacent=new Map(graph.nodes.map(n=>[n.id,[]]));
  for(const e of graph.edges){adjacent.get(e.source).push(e.target);adjacent.get(e.target).push(e.source);}
  const seen=new Set([centre]);let frontier=[centre];
  for(let hop=0;hop<radius;hop++){const next=[];for(const id of frontier)for(const id2 of adjacent.get(id))if(!seen.has(id2)){seen.add(id2);next.push(id2);}frontier=next;}
  return {nodes:graph.nodes.filter(n=>seen.has(n.id)),edges:graph.edges.filter(e=>seen.has(e.source)&&seen.has(e.target))};
}
export function mountGraph(container,initial,options={}) {
  validateGraph(initial);
  const doc=container.ownerDocument;const win=doc.defaultView;
  const canvas=doc.createElement('canvas');canvas.className='ontomark-graph-canvas';canvas.tabIndex=0;
  canvas.setAttribute('role','img');canvas.setAttribute('aria-label','Ontology graph. Drag to pan, scroll to zoom. Arrow keys select nodes; Enter opens the selected note.');
  canvas.style.cssText='display:block;width:100%;height:100%;min-height:260px;touch-action:none;outline-offset:-3px';
  const status=doc.createElement('div');status.setAttribute('role','status');status.setAttribute('aria-live','polite');
  status.style.cssText='position:absolute;left:8px;bottom:8px;font:12px sans-serif;pointer-events:none';
  const previousPosition=container.style.position;container.style.position='relative';container.append(canvas,status);
  const ctx=canvas.getContext('2d');if(!ctx){canvas.remove();status.remove();container.style.position=previousPosition;throw new Error('Canvas 2D unavailable');}
  let graph=initial,positions=seedPositions(graph),selected=null,hover=null,zoom=1,cx=0,cy=0,width=1,height=1,frame=0,dead=false,drag=null,moved=false;
  const listeners=[];let lookup=new Map(graph.nodes.map(n=>[n.id,n]));
  const listen=(type,fn,opts)=>{canvas.addEventListener(type,fn,opts);listeners.push([type,fn,opts]);};
  const scale=()=>Math.max(1,Math.min(win.devicePixelRatio||1,2));
  const screen=(p)=>({x:(p.x-cx)*zoom+width/2,y:(p.y-cy)*zoom+height/2});
  const radius=(n)=>Math.min(12,5+Math.log2(1+(n.degree??0)));
  function request(){if(!dead&&!frame)frame=win.requestAnimationFrame(draw);}
  function draw(){
    frame=0;if(dead)return;
    const dpr=scale();ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,width,height);
    const bg=options.background??win.getComputedStyle(container).getPropertyValue('--background-primary').trim()??'#fff';
    ctx.fillStyle=bg||'#fff';ctx.fillRect(0,0,width,height);
    const fg=options.foreground??win.getComputedStyle(container).getPropertyValue('--text-normal').trim()??'#333';
    for(const e of graph.edges){const a=screen(positions.get(e.source)),b=screen(positions.get(e.target));
      ctx.strokeStyle=e.color||(e.inferred?'#947ca8':'#a8a8a8');ctx.lineWidth=selected&&(e.source===selected||e.target===selected)?2:1;
      ctx.setLineDash(e.inferred?[4,4]:[]);ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.stroke();
      // Arrowhead indicates subclass/type direction without adding DOM nodes.
      const dx=b.x-a.x,dy=b.y-a.y,len=Math.hypot(dx,dy);if(len>22){const ux=dx/len,uy=dy/len,ex=b.x-ux*9,ey=b.y-uy*9;
        ctx.beginPath();ctx.moveTo(ex-ux*6+uy*3,ey-uy*6-ux*3);ctx.lineTo(ex,ey);ctx.lineTo(ex-ux*6-uy*3,ey-uy*6+ux*3);ctx.stroke();}
      if(e.kind && (options.edgeLabelMode==='on'||(options.edgeLabelMode==='hover'&&(e.source===hover||e.target===hover||e.source===selected||e.target===selected)))){ctx.font='11px sans-serif';ctx.fillStyle=fg||'#333';ctx.fillText(e.kind,(a.x+b.x)/2+4,(a.y+b.y)/2-4);}
    }
    ctx.setLineDash([]);ctx.font='12px sans-serif';ctx.textBaseline='middle';
    const ranked=[...graph.nodes].sort((a,b)=>(b.degree??0)-(a.degree??0));const labelled=new Set(ranked.slice(0,graph.nodes.length<=120?120:40).map(n=>n.id));
    for(const n of graph.nodes){const p=screen(positions.get(n.id)),r=radius(n);if(p.x<-100||p.y<-30||p.x>width+100||p.y>height+30)continue;
      ctx.fillStyle=n.color||(n.kind==='individual'?'#d49b54':n.kind==='property'?'#a382c9':'#6e9d88');ctx.beginPath();
      if(n.kind==='property'){ctx.moveTo(p.x,p.y-r);ctx.lineTo(p.x+r,p.y);ctx.lineTo(p.x,p.y+r);ctx.lineTo(p.x-r,p.y);ctx.closePath();}
      else if(n.kind==='individual')ctx.rect(p.x-r,p.y-r,2*r,2*r);else ctx.arc(p.x,p.y,r,0,Math.PI*2);ctx.fill();
      if(n.id===selected||n.id===hover){ctx.strokeStyle=fg||'#333';ctx.lineWidth=2;ctx.stroke();}
      if(labelled.has(n.id)||n.id===selected||n.id===hover){ctx.fillStyle=fg||'#333';ctx.fillText(n.label,p.x+r+5,p.y);}
    }
    const n=lookup.get(hover??selected);status.textContent=n?n.label:`${graph.nodes.length} nodes · ${graph.edges.length} edges`;
  }
  function resize(){const box=container.getBoundingClientRect();width=Math.max(1,box.width);height=Math.max(260,box.height);canvas.width=Math.round(width*scale());canvas.height=Math.round(height*scale());request();}
  function fit(){if(!graph.nodes.length){cx=cy=0;zoom=1;request();return;}
    let minX=Infinity,minY=Infinity,maxX=-Infinity,maxY=-Infinity;for(const p of positions.values()){minX=Math.min(minX,p.x);maxX=Math.max(maxX,p.x);minY=Math.min(minY,p.y);maxY=Math.max(maxY,p.y);}
    cx=(minX+maxX)/2;cy=(minY+maxY)/2;zoom=Math.max(.05,Math.min(4,Math.min((width-70)/Math.max(maxX-minX,60),(height-60)/Math.max(maxY-minY,60))));request();
  }
  function setSelection(id){if(id!==null&&!lookup.has(id))throw new Error('Unknown selected node');selected=id;request();}
  const pick=(e)=>{const rect=canvas.getBoundingClientRect();const x=e.clientX-rect.left,y=e.clientY-rect.top;let found=null,dist=Infinity;for(const n of graph.nodes){const p=screen(positions.get(n.id)),d=Math.hypot(x-p.x,y-p.y);if(d<radius(n)+7&&d<dist){found=n;dist=d;}}return found;};
  listen('pointerdown',e=>{if(e.button!==0)return;drag={x:e.clientX,y:e.clientY,startX:e.clientX,startY:e.clientY};moved=false;canvas.setPointerCapture(e.pointerId);});
  listen('pointermove',e=>{if(drag){const dx=e.clientX-drag.x,dy=e.clientY-drag.y;if(Math.hypot(e.clientX-drag.startX,e.clientY-drag.startY)>3)moved=true;cx-=dx/zoom;cy-=dy/zoom;drag={...drag,x:e.clientX,y:e.clientY};request();}else{const n=pick(e);if(hover!==(n?.id??null))options.onHover?.(n);hover=n?.id??null;canvas.title=n?.label??'';canvas.style.cursor=n?'pointer':'grab';request();}});
  listen('pointerup',e=>{if(!drag)return;drag=null;if(canvas.hasPointerCapture(e.pointerId))canvas.releasePointerCapture(e.pointerId);if(!moved){const n=pick(e);setSelection(n?.id??null);options.onSelect?.(n);}request();});
  listen('pointercancel',()=>{drag=null;});
  listen('pointerleave',()=>{if(!drag){hover=null;options.onHover?.(null);request();}});
  listen('dblclick',e=>{const n=pick(e);if(n)options.onActivate?.(n);});
  listen('wheel',e=>{e.preventDefault();const rect=canvas.getBoundingClientRect(),px=e.clientX-rect.left-width/2,py=e.clientY-rect.top-height/2;const old=zoom;zoom=Math.max(.05,Math.min(30,zoom*Math.exp(-e.deltaY*.001)));cx+=px/old-px/zoom;cy+=py/old-py/zoom;request();},{passive:false});
  listen('keydown',e=>{if(e.key==='Home'){e.preventDefault();fit();}else if(e.key==='+'||e.key==='='){e.preventDefault();zoomBy(1.25);}else if(e.key==='-'){e.preventDefault();zoomBy(.8);}else if(e.key==='Escape'){setSelection(null);options.onSelect?.(null);}else if(e.key==='Enter'){const n=lookup.get(selected);if(n)options.onActivate?.(n);}else if(['ArrowRight','ArrowLeft'].includes(e.key)&&graph.nodes.length){e.preventDefault();const i=graph.nodes.findIndex(n=>n.id===selected),next=i<0?(e.key==='ArrowRight'?0:graph.nodes.length-1):(i+(e.key==='ArrowRight'?1:-1)+graph.nodes.length)%graph.nodes.length;const n=graph.nodes[next];setSelection(n.id);options.onSelect?.(n);}});
  function zoomBy(factor){if(!Number.isFinite(factor)||factor<=0)throw new Error('Invalid zoom factor');zoom=Math.max(.05,Math.min(30,zoom*factor));request();}
  const observer=new win.ResizeObserver(resize);observer.observe(container);resize();fit();
  return {
    setGraph(next,{preserveViewport=false}={}){validateGraph(next);graph=next;lookup=new Map(next.nodes.map(n=>[n.id,n]));positions=seedPositions(next);if(selected&&!lookup.has(selected))selected=null;hover=null;if(preserveViewport)request();else fit();},
    setPositions(next){if(next.length!==graph.nodes.length*2||!Array.from(next).every(Number.isFinite))throw new Error('Position snapshot length/values invalid');graph.nodes.forEach((n,i)=>positions.set(n.id,{x:next[i*2],y:next[i*2+1]}));request();},
    setOptions(next){Object.assign(options,next);request();},
    setSelection,fit,zoomBy,getState:()=>({selected,zoom,nodes:graph.nodes.length,edges:graph.edges.length}),
    destroy(){if(dead)return;dead=true;if(frame)win.cancelAnimationFrame(frame);observer.disconnect();for(const [t,f,o] of listeners)canvas.removeEventListener(t,f,o);canvas.remove();status.remove();container.style.position=previousPosition;positions.clear();lookup.clear();},
  };
}

/** Adapt a typed engine index without minting identities or promoting prose
 * links to edges. Inference remains distinguished from asserted hierarchy. */
export function hierarchyGraph(index,includeInferred=true) {
  const nodes=index.entities.map(e=>({id:e.iri,label:e.label,kind:e.kind,degree:0}));
  const ids=new Set(nodes.map(n=>n.id));const edges=[];const seen=new Set();
  function add(source,target,kind,inferred){if(!ids.has(source)||!ids.has(target)||source===target)return;const key=JSON.stringify([source,target,kind]);if(seen.has(key))return;seen.add(key);edges.push({source,target,kind,inferred});}
  for(const e of index.entities)for(const parent of e.parents)add(e.iri,parent,e.kind==='individual'?'instance-of':e.kind==='property'?'subproperty-of':'subclass-of',false);
  if(includeInferred)for(const s of index.classification?.subsumptions??[])if(!s.told)add(s.sub,s.sup,'subclass-of',true);
  const degree=new Map(nodes.map(n=>[n.id,0]));for(const e of edges){degree.set(e.source,degree.get(e.source)+1);degree.set(e.target,degree.get(e.target)+1);}for(const n of nodes)n.degree=degree.get(n.id);
  const graph={nodes,edges};validateGraph(graph);return graph;
}
