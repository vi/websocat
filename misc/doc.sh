#!/bin/sh
python3 misc/doc.py  > doc.md # to update help_addendum.txt
crcargo build # to update the help message
python3 misc/doc.py  > doc.md
