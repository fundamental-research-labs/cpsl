# Text processing tools
# grep basic
printf "apple\nbanana\ncherry\napricot\n" | grep "ap"

# grep -i (case insensitive)
printf "Hello\nhello\nHELLO\nworld\n" | grep -i "hello"

# grep -v (invert match)
printf "apple\nbanana\ncherry\n" | grep -v "banana"

# grep -c (count)
printf "aa\nbb\naa\ncc\naa\n" | grep -c "aa"

# head
printf "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n" | head -3

# tail
printf "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n" | tail -3

# sort
printf "banana\napple\ncherry\n" | sort

# sort -r (reverse)
printf "banana\napple\ncherry\n" | sort -r

# sort -n (numeric)
printf "10\n2\n30\n1\n" | sort -n

# uniq
printf "aaa\naaa\nbbb\nccc\nccc\naaa\n" | uniq
