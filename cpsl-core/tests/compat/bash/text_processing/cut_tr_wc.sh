#!/bin/bash
# cut with -d and -f combined
echo "a,b,c" | cut -d',' -f2
echo "one:two:three" | cut -d':' -f1
echo "one:two:three" | cut -d':' -f3

# tr character translation
echo "hello" | tr 'a-z' 'A-Z'
echo "HELLO" | tr 'A-Z' 'a-z'

# tr delete mode
echo "hello world" | tr -d ' '

# wc -l
echo -n "line1
line2
line3" | wc -l

# wc -w
echo "one two three four" | wc -w

# wc -c
echo -n "hello" | wc -c
