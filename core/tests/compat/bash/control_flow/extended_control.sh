# Extended control flow
# If/elif/else
X=10
if [ "$X" -lt 5 ]; then
    echo "small"
elif [ "$X" -lt 15 ]; then
    echo "medium"
else
    echo "large"
fi

# For loop over list
for item in apple banana cherry; do
    echo "fruit: $item"
done

# For loop with numbers
for i in 1 2 3 4 5; do
    echo "num: $i"
done

# && and || chaining
true && echo "and-true"
false || echo "or-false"

# Nested if
A=5
B=10
if [ "$A" -lt "$B" ]; then
    if [ "$A" -gt 0 ]; then
        echo "A is positive and less than B"
    fi
fi
