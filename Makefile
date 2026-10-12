# dgpuj is one C file.
#
#   make                  ./dgpuj, or ./dgpuj.exe, for this machine
#   make TARGET=<triple>  for another: a Rust-style triple, as the release
#                         archives are named
#   make test             launch $JAVA_HOME through it, every way the tests know
#   make clean
#
# A Windows target builds anywhere there is clang, lld-link and llvm-dlltool;
# nothing of Windows is needed, not even its headers. The others build where
# they run. The tests are Java and compiled here, so $JAVA_HOME has to be a
# JDK; $OTHER_JAVA_HOME is a second one, a stranger for PATH.

ARCH := $(patsubst arm64,aarch64,$(patsubst amd64,x86_64,$(shell uname -m)))
OS := $(shell uname -s)
ifeq ($(OS),Linux)
HOST := $(ARCH)-unknown-linux-gnu
else ifeq ($(OS),Darwin)
HOST := $(ARCH)-apple-darwin
else
HOST := $(ARCH)-pc-windows-msvc
endif
TARGET ?= $(HOST)

WARNINGS := -Wall -Wextra -Werror

# What the binary was last built for. Two targets share a file name, and make
# tells a stale file by its date alone: without this, asking for the other
# Windows would be answered with the one already there.
.target: FORCE
	@echo '$(TARGET) $(CC)' | cmp -s - $@ || echo '$(TARGET) $(CC)' > $@
FORCE:

ifneq (,$(findstring -windows-,$(TARGET)))

BIN := dgpuj.exe

ifeq ($(TARGET),x86_64-pc-windows-msvc)
MACHINE := i386:x86-64
# One section, aligned to nothing, at a fixed address so that there is no
# relocation table to carry: what gets it into a packet.
LAYOUT := -align:4 -filealign:4 -fixed -dynamicbase:no -ignore:4108 \
	-merge:.rdata=.text -merge:.data=.text -merge:.pdata=.text \
	-section:.text,erw
else
# Windows on ARM takes none of that: an image there can always move, and one
# laid out like this it will not load. So an ordinary layout, which comes to
# a few kilobytes.
MACHINE := arm64
LAYOUT :=
endif

# kernel32's import library is made from the names dgpuj.c declares, and the
# stub is what a PE has to begin with and no program for DOS after it. The
# linker's options are spelt with a dash, not a slash, which a Windows shell
# would take for paths and rewrite.
$(BIN): dgpuj.c Makefile .target
	{ echo "LIBRARY KERNEL32.dll"; echo "EXPORTS"; \
	  sed -n 's/^IMPORT .*[ *]\([A-Za-z]*\)(.*/\1/p' dgpuj.c; } > kernel32.def
	llvm-dlltool -m $(MACHINE) -d kernel32.def -l kernel32.lib
	printf 'MZ' > stub.bin
	head -c 62 /dev/zero >> stub.bin
	clang --target=$(TARGET) -Oz -ffreestanding -fno-stack-protector \
		-mno-stack-arg-probe $(WARNINGS) -c dgpuj.c -o dgpuj.obj
	lld-link -nologo -out:$@ -entry:entry -subsystem:console -nodefaultlib \
		-stub:stub.bin -noimplib $(LAYOUT) dgpuj.obj kernel32.lib
	rm -f kernel32.def kernel32.lib dgpuj.obj stub.bin
	@echo "$@: $$(wc -c < $@ | tr -d ' ') bytes"

else

BIN := dgpuj

ifneq (,$(findstring -apple-,$(TARGET)))
FLAGS := -arch $(patsubst aarch64,arm64,$(firstword $(subst -, ,$(TARGET))))
else
# Static, so that it asks nothing of the libc it meets. Against glibc that is
# most of a megabyte; with CC=musl-gcc it is a few kilobytes, which is how a
# release is built.
FLAGS := -static
endif

$(BIN): dgpuj.c Makefile .target
	$(CC) $(FLAGS) -Os $(WARNINGS) -o $@ dgpuj.c

endif

.PHONY: all test clean FORCE
.DEFAULT_GOAL := all
all: $(BIN)

test: $(BIN)
	rm -rf test/out
	mkdir -p test/out
	"$(JAVA_HOME)/bin/javac" -nowarn -d test/out test/*.java
	"$(JAVA_HOME)/bin/java" -cp test/out Tests ./$(BIN) test/out

clean:
	rm -rf dgpuj dgpuj.exe .target test/out kernel32.def kernel32.lib dgpuj.obj stub.bin
