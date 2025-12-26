#!/bin/sh
cd doc && mdbook build && rsync -aPy book/* vi@hw:websocat4book/
