#!/bin/sh
# Build dgpuj for a target, into ./dgpuj or ./dgpuj.exe.
#
#   sh build.sh [target]
#
# The target is a Rust-style triple, as the release archives are named, and
# defaults to the machine's own. A Windows one builds anywhere there is clang,
# lld-link and llvm-dlltool; nothing of Windows is needed, not even its
# headers. The others build where they run.
set -e
cd "$(dirname "$0")"

host() {
    arch=$(uname -m)
    case "$arch" in arm64) arch=aarch64 ;; amd64) arch=x86_64 ;; esac
    case "$(uname -s)" in
        Linux) echo "$arch-unknown-linux-gnu" ;;
        Darwin) echo "$arch-apple-darwin" ;;
        *) echo "$arch-pc-windows-msvc" ;;
    esac
}
target=${1:-$(host)}

case "$target" in
*-pc-windows-msvc)
    case "$target" in
        # One section, aligned to nothing, at a fixed address so that there
        # is no relocation table to carry: what gets it into a packet.
        x86_64-*)
            machine=i386:x86-64
            layout="-align:4 -filealign:4 -fixed -dynamicbase:no -ignore:4108
                -merge:.rdata=.text -merge:.data=.text -merge:.pdata=.text
                -section:.text,erw"
            ;;
        # Windows on ARM takes none of that: an image there can always move,
        # and one laid out like this it will not load. So an ordinary layout,
        # which comes to a few kilobytes.
        aarch64-*) machine=arm64 layout= ;;
        *) echo "no such Windows: $target" >&2; exit 1 ;;
    esac

    # kernel32's import library, from the names dgpuj.c declares.
    {
        echo "LIBRARY KERNEL32.dll"
        echo "EXPORTS"
        sed -n 's/^IMPORT .*[ *]\([A-Za-z]*\)(.*/\1/p' dgpuj.c
    } > kernel32.def
    llvm-dlltool -m "$machine" -d kernel32.def -l kernel32.lib

    # What a PE has to begin with, and no program for DOS after it.
    printf 'MZ' > stub.bin
    head -c 62 /dev/zero >> stub.bin

    clang --target="$target" -Oz -ffreestanding -fno-stack-protector \
        -mno-stack-arg-probe -Wall -Wextra -Werror -c dgpuj.c -o dgpuj.obj

    # Options with a dash, not a slash, which a Windows shell would take for
    # paths and rewrite.
    lld-link -nologo -out:dgpuj.exe -entry:entry -subsystem:console -nodefaultlib \
        -stub:stub.bin -noimplib $layout \
        dgpuj.obj kernel32.lib
    rm -f kernel32.def kernel32.lib dgpuj.obj stub.bin
    echo "dgpuj.exe: $(wc -c < dgpuj.exe | tr -d ' ') bytes"
    ;;
*-apple-darwin)
    arch=${target%%-*}
    case "$arch" in aarch64) arch=arm64 ;; esac
    cc -arch "$arch" -Os -Wall -Wextra -Werror -o dgpuj dgpuj.c
    ;;
*-linux-*)
    # Static, so that it asks nothing of the libc it meets. Against glibc that
    # is most of a megabyte; with CC=musl-gcc it is a few kilobytes, which is
    # how a release is built.
    ${CC:-cc} -static -Os -Wall -Wextra -Werror -o dgpuj dgpuj.c
    ;;
*)
    echo "no such target: $target" >&2
    exit 1
    ;;
esac
