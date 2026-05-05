# Complex pipeline patterns
# Multi-stage text processing
printf "banana\napple\ncherry\napple\nbanana\napple\n" | sort | uniq

# Sort and take top
printf "5\n3\n8\n1\n9\n2\n7\n" | sort -n | tail -3

# Grep and count
printf "error: bad\ninfo: ok\nerror: fail\ninfo: good\nerror: crash\n" | grep "error" | grep -c "error"

# Pipe with head
printf "alpha\nbeta\ngamma\ndelta\nepsilon\n" | head -2

# Sort reverse and take first
printf "10\n5\n20\n15\n" | sort -rn | head -1
