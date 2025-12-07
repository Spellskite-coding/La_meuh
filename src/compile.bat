@echo off
set CC=gcc
set RES=windres

%RES% resource.rc -o resource.o
%CC% main.c resource.o -o la_meuh.exe -mwindows -luser32 -lgdi32 -lcomctl32
del resource.o
pause
