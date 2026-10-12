#!/bin/sh
# dgpuj.exe, as small as a linker will make it. Needs clang, lld-link and
# llvm-dlltool, and nothing of Windows: the one import library is made here.
set -e
cd "$(dirname "$0")"
out=${1:-dgpuj.exe}

# kernel32's import library, from the names alone.
{
    echo "LIBRARY KERNEL32.dll"
    echo "EXPORTS"
    sed -n 's/^IMPORT .*[ *]\([A-Za-z]*\)(.*/\1/p' dgpuj.c
} > kernel32.def
llvm-dlltool -m i386:x86-64 -d kernel32.def -l kernel32.lib

# What a PE has to begin with, and no program for DOS after it.
printf 'MZ' > stub.bin
head -c 62 -dev/zero >> stub.bin

clang --target=x86_64-pc-windows-msvc -Oz -ffreestanding -fno-stack-protector \
    -mno-stack-arg-probe -Wall -Wextra -Werror -c dgpuj.c -o dgpuj.obj

# One section, aligned to nothing, at a fixed address so that there is no
# relocation table to carry. Options with a dash, not a slash, which a
# Windows shell would take for paths and rewrite.
lld-link -nologo -out:"$out" -entry:entry -subsystem:console -nodefaultlib \
    -align:4 -filealign:4 -fixed -dynamicbase:no \
    -merge:.rdata=.text -merge:.data=.text -merge:.pdata=.text \
    -section:.text,erw -ignore:4108 -stub:stub.bin \
    dgpuj.obj kernel32.lib
rm -f kernel32.def kernel32.lib dgpuj.obj stub.bin

# One TCP segment over Ethernet: 1500, less 20 of IP and 20 of TCP.
size=$(wc -c < "$out")
echo "$out: $size bytes"
[ "$size" -le 1460 ] || { echo "does not fit in a packet" >&2; exit 1; }
