#!/usr/bin/env python3
"""Add explanatory titles/subtitles to an unaltered synchronized recording."""
import argparse,hashlib,json,subprocess
from pathlib import Path
from run import ROOT

def stamp(seconds):
 ms=max(0,round(seconds*1000));return f'{ms//3600000:02}:{ms//60000%60:02}:{ms//1000%60:02},{ms%1000:03}'
def main():
 p=argparse.ArgumentParser();p.add_argument('--recording',type=Path,required=True);p.add_argument('--output',type=Path,default=ROOT/'docs/demo/jev-web');p.add_argument('--poster-at',type=float);a=p.parse_args();a.recording=a.recording.resolve();a.output=a.output.resolve();a.output.mkdir(parents=True,exist_ok=True)
 timeline=json.loads((a.recording/'timeline.json').read_text());start=json.loads((a.recording/'capture-start.json').read_text())['time']
 raw=a.recording/'raw.mp4';duration=float(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','default=nw=1:nk=1',str(raw)],text=True))
 result=json.loads((a.recording/'result.json').read_text());verified=sum(r['passed'] for r in result)
 phases={'INTRO':'左：原网页 GUI；中：真实 GUI2TUI；右：JEV 结构化判断。', 'OBSERVING':'观察当前公共 Accessibility 状态，获取合法操作候选。', 'JEV IS DECIDING':'JEV 从候选 ID 中选择操作；填写文字由任务参数提供。', 'SELECTED / VALIDATING':'模型给出选择；G2T 执行前重新校验身份、代次和作用域。', 'READING BACK':'读取权威 GUI 状态：派发接受与任务效果分别验证。', 'VERIFIED':'独立断言通过；GUI、TUI 与执行结果保持真实记录。', 'WAITING':'JEV 选择等待；重新读取页面，不重放上一个操作。', 'MODEL CLAIMS DONE':'JEV 判断任务完成；仍须独立核验实际页面。', 'NOT CONFIRMED':'当前结果尚未通过独立验证，如实保留。', 'COMPLETE':f'演示完成：{verified}/{len(result)} 个任务通过独立验证。JEV 选择，G2T 校验与执行。'}
 captions=[(0,min(4,duration),phases['INTRO'])]
 for i,event in enumerate(timeline):
  begin=max(4,event['time']-start)
  end=min(duration,timeline[i+1]['time']-start if i+1<len(timeline) else duration)
  if end>begin+.2:captions.append((begin,end,phases.get(event['phase'],event['phase'])))
 srt=a.output/'captions.zh.srt';srt.write_text('\n\n'.join(f'{i}\n{stamp(begin)} --> {stamp(end)}\n{text}' for i,(begin,end,text) in enumerate(captions,1))+'\n')
 font='/System/Library/Fonts/STHeiti Medium.ttc'
 filters=['pad=1920:1080:0:60:color=0x0b1220','drawbox=x=638:y=60:w=4:h=1000:color=0x334155:t=fill','drawbox=x=1278:y=60:w=4:h=1000:color=0x334155:t=fill']
 for index,title in enumerate(['GUI | 原网页：权威状态','GUI2TUI | 实时语义终端','JEV | 判断与执行回执']):
  file=a.output/f'title-{index}.txt';file.write_text(title)
  filters.append(f"drawtext=fontfile='{font}':textfile='{file.resolve()}':x={index*640+18}:y=16:fontsize=24:fontcolor=white")
 filters.append(f"subtitles=filename='{srt.resolve()}':fontsdir='/System/Library/Fonts':force_style='FontName=Heiti SC,FontSize=8,BorderStyle=3,OutlineColour=&H90000000,Alignment=2,MarginV=4'")
 output=a.output/'gui-tui-jev.mp4'
 subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(raw),'-vf',','.join(filters),'-c:v','libx264','-preset','fast','-crf','21','-pix_fmt','yuv420p','-movflags','+faststart',str(output)],check=True)
 subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-ss',str(a.poster_at if a.poster_at is not None else duration*.55),'-i',str(output),'-frames:v','1',str(a.output/'poster.png')],check=True)
 result=json.loads((a.recording/'result.json').read_text())
 metadata={'duration_s':duration,'resolution':'1920x1080','layout':['original GUI','actual GUI2TUI PTY','real JEV choice/receipt/oracle'],
 'verified_steps':sum(r['passed'] for r in result),'total_steps':len(result),
 'verified_tasks':verified,'total_tasks':len(result),'executed_gui_operations':sum(r.get('gui_writes',1) for r in result),
 'video_sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'raw_sha256':hashlib.sha256(raw.read_bytes()).hexdigest(),
 'gui2tui_sha256':hashlib.sha256((a.recording/'gui2tui').read_bytes()).hexdigest(), 'agent_sha256':hashlib.sha256((a.recording/'gui2tui-agent').read_bytes()).hexdigest(),
 'raw_evidence':str(a.recording.relative_to(ROOT)),'semantic_source':'public Accessibility only; pixels are display evidence',
 'capture':'Two virtual X11 screens captured synchronously and horizontally stacked to avoid presentation terminals interfering with GUI focus.'}
 if (a.recording/'task.json').exists():metadata['task']=json.loads((a.recording/'task.json').read_text())
 (a.output/'recording.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2))
 print(json.dumps(metadata,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
