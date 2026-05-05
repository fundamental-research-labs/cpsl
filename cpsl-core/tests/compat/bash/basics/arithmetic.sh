# Arithmetic expansion $((expr))
echo $((1 + 2))
echo $((10 - 3))
echo $((4 * 5))
echo $((10 / 3))
echo $((10 % 3))

# Variables in arithmetic
x=7
echo $((x + 3))
echo $((x * 2))

# Nested arithmetic
echo $((2 + 3 * 4))
echo $(((2 + 3) * 4))

# While loop with arithmetic counter
i=0
while [ "$i" -lt 3 ]; do
  echo "count $i"
  i=$((i + 1))
done

# Until loop with arithmetic
j=5
until [ "$j" -le 2 ]; do
  echo "until $j"
  j=$((j - 1))
done
