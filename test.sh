#!/bin/sh
# Launch a real JDK through dgpuj, every way the tests know.
#
#   sh test.sh [dgpuj]
#
# The JDK is $JAVA_HOME, and has to be a JDK, the tests being Java and compiled
# here. $OTHER_JAVA_HOME is a second one, a stranger for PATH.
set -e
cd "$(dirname "$0")"
dgpuj=${1:-./dgpuj}
[ -f "$dgpuj" ] || dgpuj=$dgpuj.exe

rm -rf test/out
mkdir -p test/out
"$JAVA_HOME/bin/javac" -nowarn -d test/out test/*.java
exec "$JAVA_HOME/bin/java" -cp test/out Tests "$dgpuj" test/out
