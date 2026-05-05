# Bash functions
# Basic function
greet() {
    echo "hello $1"
}
greet "world"
greet "alice"

# Function with multiple args
show_args() {
    echo "first: $1 second: $2"
}
show_args "a" "b"

# Function called multiple times
say() {
    echo "saying: $1"
}
say "one"
say "two"
say "three"
