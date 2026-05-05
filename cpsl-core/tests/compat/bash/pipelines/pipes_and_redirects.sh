# Pipes and command substitution
# Multi-stage pipe
printf "cherry\napple\nbanana\n" | sort
printf "aaa\nbbb\naaa\nccc\nbbb\naaa\n" | sort | uniq

# Pipe with grep
printf "hello world\nfoo bar\nhello again\n" | grep "hello"

# Pipe chain
printf "3\n1\n4\n1\n5\n9\n" | sort -n | head -3

# Heredoc
cat <<EOF
line one
line two
line three
EOF

# Command substitution
RESULT=$(echo "captured")
echo "got: $RESULT"
