@echo off
REM Lang-Zone 开发循环任务执行脚本
REM 每45分钟执行一次

cd /d F:\AI\lang-zone

REM 读取元提示词并执行
codearts run -m huaweicloud-maas/openpangu-2.0-pro --agent build "Read _meta_prompt.md and execute the workflow defined in it" --title "LangZone-DevLoop"
