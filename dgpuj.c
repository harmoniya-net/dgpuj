/*
 * dgpuj — `java`, on the machine's better graphics card.
 *
 *   dgpuj <java> <whatever java takes>
 *
 * `<java>` is the `java` to be: a path to one, `C:\jdk\bin\java.exe`, or the
 * bare word, for the one on PATH. dgpuj goes in front of a launch line the
 * way `nice` does.
 *
 * Which GPU a program gets is decided for the process that creates the GL or
 * D3D context, and on Windows by looking at that process's executable. So
 * starting `java.exe` from something else changes nothing there: the game is
 * drawn by `java.exe`, which carries no hint. On Windows dgpuj is therefore
 * that `java` itself, with the two symbols a driver looks for exported from
 * the executable. Elsewhere the hint is environment or there is none, so
 * dgpuj sets it and runs the rest of the line.
 *
 * Built by ./build.sh.
 */

#ifdef _WIN32

/*
 * `java.exe` is one call, `JLI_Launch`, into its JDK's `jli.dll`, and dgpuj
 * makes the same call. It is also small enough to fit in one TCP packet,
 * which nothing needed and which shaped it all the same: what it does not do
 * itself it asks `jli.dll` for, and that turned out to be nearly everything.
 *
 * - No C runtime: no `main`, no `malloc`, no `printf`. Memory is
 *   `JLI_MemAlloc`.
 * - No parsing of arguments. `JLI_CmdToArgs` splits the command line, which is
 *   how `java.exe` gets its own `argv`; all that is read here is its first
 *   word, which says where `jli.dll` is, and it is cut out again before that.
 * - No Unicode, the launcher reading the command line as ANSI anyway.
 * - No looking at files, and no looking for `java`. A JDK is there if its
 *   `jli.dll` loads, and finding the one on PATH is asking Windows for
 *   `jli.dll` by name alone, which looks beside dgpuj first: so the same
 *   word serves a dgpuj installed in a JDK's `bin`, where Java 8 needs it.
 *
 * It gives up being told what went wrong. With no JDK where `<java>` says
 * there is nothing loaded to report it with, so it exits with 1 and says
 * nothing; and a Java 8 that dgpuj is not beside fails in the JDK's words,
 * "could not find java.dll", not ours. Java 8 finds its own files from where
 * its launcher is and nothing else; `jli.dll` learned to look beside itself
 * in Java 9.
 */

typedef unsigned long DWORD;
typedef void *HANDLE;
#define IMPORT __declspec(dllimport)

IMPORT char *GetCommandLineA(void);
IMPORT int SetDllDirectoryA(const char *);
IMPORT HANDLE LoadLibraryA(const char *);
IMPORT void *GetProcAddress(HANDLE, const char *);
IMPORT void ExitProcess(unsigned);

/* Read by the Optimus / PowerXpress driver from the export table of the
 * executable that creates the context. 1 is "high performance graphics". */
__declspec(dllexport) DWORD NvOptimusEnablement = 1;
__declspec(dllexport) DWORD AmdPowerXpressRequestHighPerformance = 1;

#define PATH 260

/* One of `jli.dll`'s own arguments, as `JLI_GetStdArgs` hands them out. */
typedef struct { char *arg; char has_wildcard; } StdArg;

typedef int (*JliLaunch)(int, char **, int, const char **, int, const char **,
                         const char *, const char *, const char *, const char *,
                         unsigned char, unsigned char, unsigned char, int);

static void put(char *to, const char *from)
{
    while ((*to++ = *from++))
        ;
}

void entry(void)
{
    static const char java[] = "java";
    /* `<java>`, with room for the `\\jli.dll` that takes its last word's place. */
    char bin[PATH + 16];
    char *line = GetCommandLineA(), *c = line, *word, *end = bin, *name = bin;
    char stop, **argv;
    HANDLE jli;
    StdArg *args;
    int argc, i;

    /* Past the program's own name, to the `java` it is to be. */
    stop = *c == '"' ? '"' : ' ';
    do c++; while (*c && *c != stop);
    if (*c) c++;
    while (*c == ' ') c++;
    word = c;

    /* Copied with its slashes turned the way `LoadLibrary` wants them, and
     * with a note of where its directory ends, if it has one. */
    stop = *c == '"' ? (c++, '"') : ' ';
    while (*c && *c != stop && end < bin + PATH) {
        char ch = *c++;
        if (ch == '/') ch = '\\';
        if (ch == '\\') name = end;
        *end++ = ch;
    }
    if (*c == '"') c++;

    /* `jli.dll` is not to see that word. It splits the command line and keeps
     * count of what it split, to tell `main`'s arguments from `java`'s, and
     * would take a word with no dash for the main class. */
    put(word, c);

    /* The JDK's DLLs import each other by bare name, and Windows looks a
     * bare name up beside the executable: make that the JDK's `bin`, or
     * another Java on PATH lends its `java.dll`. For a bare `java` this is
     * the empty string, which takes the current directory out of the search
     * and leaves the rest: beside dgpuj, then the system's, then PATH. */
    *name = 0;
    SetDllDirectoryA(bin);
    put(name, &"\\jli.dll"[name == bin]);
    jli = LoadLibraryA(bin);
    if (!jli)
        ExitProcess(1);

    /* `argv` as `java.exe` gets it: split by the JDK. */
    ((void (*)(char *))GetProcAddress(jli, "JLI_CmdToArgs"))(line);
    argc = ((int (*)(void))GetProcAddress(jli, "JLI_GetStdArgc"))();
    args = ((StdArg *(*)(void))GetProcAddress(jli, "JLI_GetStdArgs"))();
    argv = ((char **(*)(unsigned long long))GetProcAddress(jli, "JLI_MemAlloc"))(
        (argc + 1) * sizeof *argv);
    for (i = 0; i < argc; i++)
        argv[i] = args[i].arg;
    argv[argc] = 0;

    /* The constants `java.exe` is built with: no arguments of its own,
     * wildcards in the class path expanded, a console. Its names for itself
     * are only ever printed, and its version only compared with the
     * registry's by a Java 8 that has already failed to find itself. */
    ExitProcess(((JliLaunch)GetProcAddress(jli, "JLI_Launch"))(
        argc, argv, 0, 0, 0, 0, java, java, java, java, 0, 1, 0, 0));
}

#else /* Linux and macOS */

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv)
{
    if (argc < 2) {
        fputs("usage: dgpuj <java> <whatever java takes>\n", stderr);
        return 2;
    }

#ifdef __linux__
    /* NVIDIA PRIME render offload: read from the environment by the GL and
     * Vulkan loaders when the context is made. Only where the proprietary
     * driver is loaded, since `__GLX_VENDOR_LIBRARY_NAME=nvidia` without it
     * leaves GLVND with no vendor library and breaks GL; and never over what
     * the caller set, which is what the 0 says. */
    if (access("/proc/driver/nvidia/version", F_OK) == 0) {
        setenv("__NV_PRIME_RENDER_OFFLOAD", "1", 0);
        setenv("__GLX_VENDOR_LIBRARY_NAME", "nvidia", 0);
        setenv("__VK_LAYER_NV_optimus", "NVIDIA_only", 0);
    }
#endif
    /* macOS has nothing to set: Apple Silicon has one GPU, and the Intel
     * lever was an `.app`'s `Info.plist`, out of a bare binary's reach. */

    /* Nothing needs dgpuj to stay the process, so it becomes `java`. */
    execvp(argv[1], argv + 1);
    perror(argv[1]);
    return 1;
}

#endif
