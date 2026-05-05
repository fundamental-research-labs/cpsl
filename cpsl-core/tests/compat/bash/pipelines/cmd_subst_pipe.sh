# Command substitution with pipes
result=$(echo "hello world" | tr "h" "H")
echo "$result"

# Command substitution with wc pipe
count=$(echo "one two three" | wc -w)
echo "words: $count"
