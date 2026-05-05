# Extended variable tests
# Basic assignment and expansion
X=hello
Y=world
echo "$X $Y"

# Variable in double quotes
MSG="the value is $X"
echo "$MSG"

# Brace expansion
echo "${X}_suffix"
echo "prefix_${Y}"

# Variable reassignment
X=goodbye
echo "$X"

# Command substitution
RESULT=$(echo "computed value")
echo "$RESULT"
