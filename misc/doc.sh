#!/bin/sh

if [ ! -f misc/scan_source.py ]; then
   echo "Wrong current directory"
   exti 1
fi

python3 misc/scan_source.py > outline.json
python3 misc/doc.py  > doc.md  < outline.json # to update help_addendum.txt
crcargo build # to update the help message
python3 misc/doc.py  > doc.md <  outline.json
