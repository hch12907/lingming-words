#!/bin/sh

python check_dupe.py yuling_sc.lastroot.words.dict.yaml >hch.txt
python check_dupe.py yuling_tc.lastroot.words.dict.yaml >hch_tc.txt
python check_dupe.py yuling_sc.words.dict.yaml >official.txt
python check_dupe.py yuling_tc.words.dict.yaml >official_tc.txt
python check_dupe.py yuling_sc.simple.words.dict.yaml >simple.txt
python check_dupe.py yuling_tc.simple.words.dict.yaml >simple_tc.txt
python check_dupe.py yuling_sc.lastrootplus.words.dict.yaml >lastrootplus.txt
python check_dupe.py yuling_tc.lastrootplus.words.dict.yaml >lastrootplus_tc.txt
python check_dupe.py yuling_sc.lastrootdouble.words.dict.yaml >lastrootdouble.txt
python check_dupe.py yuling_tc.lastrootdouble.words.dict.yaml >lastrootdouble_tc.txt