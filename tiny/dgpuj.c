/*
 * dgpuj for Windows, small enough to fit in one TCP packet.
 *
 * The same program as ../dgpuj.c: `java.exe` with the two symbols a GPU
 * driver looks for. What makes it small is what it does without, and whom it
 * asks instead, which is nearly always the JDK's own `jli.dll`:
 *
 * - No C runtime: no `main`, no `malloc`, no `printf`. Memory is
 *   `JLI_MemAlloc` and an error is `JLI_ReportErrorMessage`.
 * - No parsing of arguments. `JLI_CmdToArgs` splits the command line, which is
 *   how `java.exe` gets its own `argv`; all that is read here is the one flag
 *   that says where `jli.dll` is.
 * - No Unicode, the launcher reading the command line as ANSI anyway.
 * - No looking at files, and no asking where dgpuj itself is. Whether a JDK
 *   is there is whether its `jli.dll` loads; whether it is Java 8 is whether
 *   that lacks a function Java 9 added; and whether dgpuj is in that JDK's
 *   `bin` is whether the `jli.dll` beside dgpuj is the same one.
 *
 * It gives up two things for it. There is no `JAVA_HOME` to fall back on: the
 * JDK is `--dgpuj-home`, or the one dgpuj is in the `bin` of. And with no JDK
 * found there is nothing loaded to report it with, so it exits with 1 and
 * says nothing.
 *
 * Built by ./build.sh, with a linker told to align nothing.
 */

typedef unsigned long DWORD;
typedef void *HANDLE;
#define IMPORT __declspec(dllimport)

IMPORT char *GetCommandLineA(void);
IMPORT int SetDllDirectoryA(const char *);
IMPORT HANDLE LoadLibraryExA(const char *, HANDLE, DWORD);
IMPORT void *GetProcAddress(HANDLE, const char *);
IMPORT void ExitProcess(unsigned);

/* Read by the Optimus / PowerXpress driver from the export table of the
 * executable that creates the context. 1 is "high performance graphics". */
__declspec(dllexport) DWORD NvOptimusEnablement = 1;
__declspec(dllexport) DWORD AmdPowerXpressRequestHighPerformance = 1;

#define PATH 260
/* LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32: beside
 * dgpuj and nowhere else, `PATH` least of all. */
#define BESIDE 0xA00
/* LOAD_WITH_ALTERED_SEARCH_PATH: what the library needs is looked for where
 * the library is. */
#define ITS_OWN 8

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
    char bin[PATH + 16];
    char *line = GetCommandLineA(), *c = line, *end = bin, stop, **argv;
    const char *flag = "--dgpuj-home ";
    /* The JDK dgpuj is in the `bin` of, as `java.exe` is, if it is in one. */
    HANDLE jli = LoadLibraryExA("jli.dll", 0, BESIDE);
    StdArg *args;
    int argc, skip = 0, i;

    /* Past the program's own name, to what may be dgpuj's one flag. */
    stop = *c == '"' ? '"' : ' ';
    do c++; while (*c && *c != stop);
    if (*c) c++;
    while (*c == ' ') c++;
    while (*flag && *c == *flag) c++, flag++;

    if (!*flag) {
        /* --dgpuj-home DIR: `jli.dll` cannot be asked, being what it finds. */
        skip = 2;
        while (*c == ' ') c++;
        stop = *c == '"' ? (c++, '"') : ' ';
        while (*c && *c != stop && end < bin + PATH) *end++ = *c++;
        put(end, "\\bin\\jli.dll");
        jli = LoadLibraryExA(bin, 0, ITS_OWN);

        /* The JDK's DLLs import each other by bare name, and Windows looks a
         * bare name up beside the executable: make that the JDK's `bin`, or
         * another Java on PATH lends its `java.dll`. */
        end[4] = 0;
        SetDllDirectoryA(bin);
    }
    if (!jli)
        ExitProcess(1);

    /* `argv` as `java.exe` gets it: split by the JDK, less dgpuj's flag. */
    ((void (*)(char *))GetProcAddress(jli, "JLI_CmdToArgs"))(line);
    argc = ((int (*)(void))GetProcAddress(jli, "JLI_GetStdArgc"))() - skip;
    args = ((StdArg *(*)(void))GetProcAddress(jli, "JLI_GetStdArgs"))();
    argv = ((char **(*)(unsigned long long))GetProcAddress(jli, "JLI_MemAlloc"))(
        (argc + 1) * sizeof *argv);
    argv[0] = args[0].arg;
    for (i = 1; i < argc; i++)
        argv[i] = args[i + skip].arg;
    argv[argc] = 0;

    /* The constants `java.exe` is built with: no arguments of its own,
     * wildcards in the class path expanded, a console. Its names for itself
     * are only ever printed. */
    ExitProcess(((JliLaunch)GetProcAddress(jli, "JLI_Launch"))(
        argc, argv, 0, 0, 0, 0, java, "0.0", java, java, 0, 1, 0, 0));
}
