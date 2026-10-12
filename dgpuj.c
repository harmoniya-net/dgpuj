/*
 * dgpuj — `java`, on the machine's better graphics card. The whole of it, in C.
 *
 *   dgpuj [--dgpuj-home DIR] <whatever java takes>
 *
 * Which GPU a program gets is decided for the process that creates the GL or
 * D3D context, and on Windows by looking at that process's executable. So
 * there dgpuj is `java.exe` with the hint: it exports the two symbols a driver
 * looks for and makes the one call `java.exe` makes, `JLI_Launch`, into the
 * JDK's `jli.dll`. On Linux the hint is environment, which survives an `exec`,
 * and on macOS there is none, so on both it becomes the JDK's own `java`.
 *
 * The JDK is `--dgpuj-home`, else the one dgpuj is installed in the `bin` of,
 * else `$JAVA_HOME`.
 *
 *   cl /nologo /O1 /MT /W4 dgpuj.c        Windows; /MT so that it needs no
 *                                         VCRUNTIME140.dll beside it
 *   cc -Os -Wall -Wextra -o dgpuj dgpuj.c elsewhere
 */

#ifdef _WIN32

#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>

/* Read by the Optimus / PowerXpress driver from the export table of the
 * executable that creates the context. 1 is "high performance graphics". */
__declspec(dllexport) DWORD NvOptimusEnablement = 1;
__declspec(dllexport) DWORD AmdPowerXpressRequestHighPerformance = 1;

/* The longest path Windows has, in characters. */
#define PATH_CAP 32768

/* `JLI_Launch`, as `jli.dll` has exported it since Java 6 (`java.h`). */
typedef int (__stdcall *JliLaunch)(
    int argc, char **argv,
    int jargc, const char **jargv,
    int appclassc, const char **appclassv,
    const char *fullversion, const char *dotversion,
    const char *pname, const char *lname,
    unsigned char javaargs, unsigned char cpwildcard,
    unsigned char javaw, int ergo);

static int fail(const wchar_t *what, const wchar_t *subject)
{
    fwprintf(stderr, L"dgpuj: %ls%ls\n", what, subject);
    return 1;
}

static int fail_os(const wchar_t *what, const wchar_t *subject)
{
    fwprintf(stderr, L"dgpuj: %ls%ls (error %lu)\n", what, subject, GetLastError());
    return 1;
}

/* `dir\name` into a buffer of its own, which is never freed: there are a
 * handful, and the process is about to become the game. */
static wchar_t *join(const wchar_t *dir, const wchar_t *name)
{
    size_t size = wcslen(dir) + wcslen(name) + 2;
    wchar_t *path = malloc(size * sizeof(wchar_t));
    if (path == NULL)
        exit(fail(L"out of memory", L""));
    _snwprintf_s(path, size, _TRUNCATE, L"%ls\\%ls", dir, name);
    return path;
}

static int is_file(const wchar_t *path)
{
    DWORD attributes = GetFileAttributesW(path);
    return attributes != INVALID_FILE_ATTRIBUTES
        && !(attributes & FILE_ATTRIBUTE_DIRECTORY);
}

/* The directory this executable is in, in full. */
static wchar_t *own_dir(void)
{
    wchar_t *path = malloc(PATH_CAP * sizeof(wchar_t));
    wchar_t *slash;
    DWORD length;
    if (path == NULL)
        return NULL;
    length = GetModuleFileNameW(NULL, path, PATH_CAP);
    if (length == 0 || length >= PATH_CAP)
        return NULL;
    slash = wcsrchr(path, L'\\');
    if (slash == NULL)
        return NULL;
    *slash = L'\0';
    return path;
}

/* The JDK dgpuj is installed in, if it sits in one's `bin` as `java.exe` does. */
static wchar_t *own_home(void)
{
    wchar_t *dir = own_dir();
    wchar_t *slash;
    if (dir == NULL || !is_file(join(dir, L"java.exe")))
        return NULL;
    slash = wcsrchr(dir, L'\\');
    if (slash == NULL)
        return NULL;
    *slash = L'\0';
    return dir;
}

static int same_dir(const wchar_t *a, const wchar_t *b)
{
    static wchar_t full_a[PATH_CAP], full_b[PATH_CAP];
    if (a == NULL
        || GetFullPathNameW(a, PATH_CAP, full_a, NULL) == 0
        || GetFullPathNameW(b, PATH_CAP, full_b, NULL) == 0)
        return 0;
    return _wcsicmp(full_a, full_b) == 0;
}

/* An argument in the system's ANSI code page, which is what `java.exe` is
 * given and what the launcher reads. Ours is Unicode, and UTF-8 bytes read as
 * cp1251 turn a class path under a Cyrillic user name into a directory that
 * does not exist. A character the code page lacks becomes its best fit or
 * `?`, exactly what `java.exe` would have been handed. */
static char *ansi(const wchar_t *arg)
{
    int size = WideCharToMultiByte(CP_ACP, 0, arg, -1, NULL, 0, NULL, NULL);
    char *bytes = size > 0 ? malloc((size_t)size) : NULL;
    if (bytes == NULL
        || WideCharToMultiByte(CP_ACP, 0, arg, -1, bytes, size, NULL, NULL) == 0)
        exit(fail_os(L"encoding ", arg));
    return bytes;
}

int wmain(int argc, wchar_t **argv)
{
    const wchar_t *home = NULL;
    wchar_t *bin, *jli;
    HMODULE module;
    JliLaunch launch;
    char **java_argv;
    int first = 1, java_argc, i;

    /* dgpuj's own flag, off the front only: the rest is `java`'s. */
    while (first < argc) {
        if (wcscmp(argv[first], L"--dgpuj-home") == 0) {
            if (first + 1 >= argc)
                return fail(L"--dgpuj-home needs a directory", L"");
            home = argv[first + 1];
            first += 2;
        } else if (wcscmp(argv[first], L"--dgpuj-jvm") == 0) {
            return fail(L"--dgpuj-jvm is gone: name the JDK with --dgpuj-home", L"");
        } else {
            break;
        }
    }
    if (home == NULL)
        home = own_home();
    if (home == NULL)
        home = _wgetenv(L"JAVA_HOME");
    if (home == NULL)
        return fail(L"no JDK: pass --dgpuj-home or set JAVA_HOME", L"");

    /* `jli.dll` is in `bin`, or in a Java 8 JDK in its `jre\bin`. */
    bin = join(home, L"bin");
    if (!is_file(join(bin, L"jli.dll"))) {
        bin = join(home, L"jre\\bin");
        if (!is_file(join(bin, L"jli.dll")))
            return fail(L"no bin\\jli.dll: no JDK at ", home);
    }

    /* Java 8 finds its own files from where its launcher is, and nothing
     * else: `jli.dll` learned to look beside itself in Java 9. So for Java 8
     * dgpuj has to be where `java.exe` is, and saying so beats the launcher's
     * "could not find java.dll". */
    if (!is_file(join(home, L"lib\\modules"))
        && !same_dir(own_dir(), join(home, L"bin")))
        return fail(L"Java 8 finds itself from where its launcher is: "
                    L"install dgpuj.exe into ", join(home, L"bin"));

    /* The JDK's DLLs import each other by bare name, and Windows looks a
     * bare name up beside the *executable*, which under `java.exe` is this
     * directory. Without it the search goes on to `PATH`: with a
     * `-javaagent` another Java installed on the machine lent its
     * `java.dll`, and the first DNS lookup died with "platform encoding not
     * initialized". */
    if (!SetDllDirectoryW(bin))
        return fail_os(L"adding to the DLL search path: ", bin);

    jli = join(bin, L"jli.dll");
    module = LoadLibraryW(jli);
    if (module == NULL)
        return fail_os(L"loading ", jli);
    launch = (JliLaunch)(void *)GetProcAddress(module, "JLI_Launch");
    if (launch == NULL)
        return fail(L"no JLI_Launch in ", jli);

    /* `argv`, as `java.exe` would have had it: the program, then the
     * arguments, then a null. */
    java_argc = argc - first + 1;
    java_argv = malloc(((size_t)java_argc + 1) * sizeof(char *));
    if (java_argv == NULL)
        return fail(L"out of memory", L"");
    java_argv[0] = "java";
    for (i = 1; i < java_argc; i++)
        java_argv[i] = ansi(argv[first + i - 1]);
    java_argv[java_argc] = NULL;

    /* The constants `java.exe` is built with (`main.c`, `defines.h`): no
     * arguments of its own, wildcards in the class path expanded, a console. */
    return launch(java_argc, java_argv, 0, NULL, 0, NULL,
                  "dgpuj", "0.0", "java", "openjdk", 0, 1, 0, 0);
}

#else /* Linux and macOS */

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#ifdef __APPLE__
#include <mach-o/dyld.h>
#endif

static int fail(const char *what, const char *subject)
{
    fprintf(stderr, "dgpuj: %s%s\n", what, subject);
    return 1;
}

static char *java_in(const char *home)
{
    size_t size = strlen(home) + sizeof "/bin/java";
    char *path = malloc(size);
    if (path == NULL)
        exit(fail("out of memory", ""));
    snprintf(path, size, "%s/bin/java", home);
    return path;
}

/* Where this executable really is, links followed. */
static char *own_path(void)
{
    static char real[PATH_MAX];
#ifdef __APPLE__
    char given[PATH_MAX];
    uint32_t size = sizeof given;
    if (_NSGetExecutablePath(given, &size) != 0)
        return NULL;
    return realpath(given, real);
#else
    return realpath("/proc/self/exe", real);
#endif
}

/* The JDK dgpuj is installed in, if it sits in one's `bin` as `java` does. */
static char *own_home(void)
{
    char *path = own_path();
    char *slash;
    int level;
    if (path == NULL)
        return NULL;
    for (level = 0; level < 2; level++) {
        slash = strrchr(path, '/');
        if (slash == NULL)
            return NULL;
        *slash = '\0';
    }
    return access(java_in(path), X_OK) == 0 ? path : NULL;
}

#ifdef __linux__
/* Opt into NVIDIA PRIME render offload, but only when the proprietary driver
 * is loaded, and never over what the caller already set (the 0). On a machine
 * without it `__GLX_VENDOR_LIBRARY_NAME=nvidia` would leave GLVND with no
 * vendor library and break GL. */
static void force_gpu(void)
{
    if (access("/proc/driver/nvidia/version", F_OK) != 0)
        return;
    setenv("__NV_PRIME_RENDER_OFFLOAD", "1", 0);
    setenv("__GLX_VENDOR_LIBRARY_NAME", "nvidia", 0);
    setenv("__VK_LAYER_NV_optimus", "NVIDIA_only", 0);
}
#else
/* macOS: Apple Silicon has one GPU, and the Intel lever was an `.app`'s
 * `Info.plist`, out of a bare binary's reach. */
static void force_gpu(void) {}
#endif

int main(int argc, char **argv)
{
    const char *home = NULL;
    char *java;
    int first = 1;

    /* dgpuj's own flag, off the front only: the rest is `java`'s. */
    while (first < argc) {
        if (strcmp(argv[first], "--dgpuj-home") == 0) {
            if (first + 1 >= argc)
                return fail("--dgpuj-home needs a directory", "");
            home = argv[first + 1];
            first += 2;
        } else if (strcmp(argv[first], "--dgpuj-jvm") == 0) {
            return fail("--dgpuj-jvm is gone: name the JDK with --dgpuj-home", "");
        } else {
            break;
        }
    }
    if (home == NULL)
        home = own_home();
    if (home == NULL)
        home = getenv("JAVA_HOME");
    if (home == NULL)
        return fail("no JDK: pass --dgpuj-home or set JAVA_HOME", "");

    force_gpu();

    /* Become `java`: its path where dgpuj's own name was, then what is left
     * of the arguments, whose terminating null `argv` already has. */
    java = java_in(home);
    argv[first - 1] = java;
    execv(java, argv + first - 1);
    perror("dgpuj: starting java");
    return 1;
}

#endif
