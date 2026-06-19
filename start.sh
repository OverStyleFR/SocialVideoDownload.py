#!/bin/bash
cd /home/container || exit 1
exec venv/bin/python main.py
