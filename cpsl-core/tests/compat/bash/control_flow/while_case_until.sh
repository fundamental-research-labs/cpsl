# case pattern matching
fruit="apple"
case "$fruit" in
  apple)
    echo "It is an apple"
    ;;
  banana)
    echo "It is a banana"
    ;;
  *)
    echo "Unknown fruit"
    ;;
esac

# case with multiple values
val="b"
case "$val" in
  a)
    echo "first"
    ;;
  b)
    echo "second"
    ;;
  c)
    echo "third"
    ;;
esac

# case with wildcard patterns
ext="file.txt"
case "$ext" in
  *.txt)
    echo "text file"
    ;;
  *.py)
    echo "python file"
    ;;
  *)
    echo "other"
    ;;
esac
