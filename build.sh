#!/bin/bash
cd ~/tfa-protector-bot
time jsub -N build -mem 2G -sync y -cwd cargo build --release
