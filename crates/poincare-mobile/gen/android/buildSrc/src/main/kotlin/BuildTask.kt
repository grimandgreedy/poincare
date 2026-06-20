import java.io.File
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction

open class BuildTask : DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun build() {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")

        val home = System.getProperty("user.home")
        val cargoHome = System.getenv("CARGO_HOME") ?: "$home/.cargo"
        val cargoExe = listOf(
            "$cargoHome/bin/cargo",
            "$home/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo",
            "/usr/local/bin/cargo",
            "/opt/homebrew/bin/cargo",
        ).firstOrNull { File(it).exists() } ?: "cargo"

        val androidHome = System.getenv("ANDROID_HOME")
            ?: "${home}/Library/Android/sdk"
        val ndkHome = System.getenv("NDK_HOME")
            ?: File("$androidHome/ndk").listFiles()
                ?.maxByOrNull { it.name }?.absolutePath
            ?: "$androidHome/ndk"

        project.exec {
            workingDir(File(project.projectDir, rootDirRel))
            val existingPath = System.getenv("PATH") ?: ""
            environment("PATH", "$cargoHome/bin:$existingPath")
            environment("ANDROID_HOME", androidHome)
            environment("NDK_HOME", ndkHome)
            executable(cargoExe)
            args(listOf("android", "build"))
            if (project.logger.isEnabled(LogLevel.DEBUG)) {
                args("-vv")
            } else if (project.logger.isEnabled(LogLevel.INFO)) {
                args("-v")
            }
            if (release) {
                args("--release")
            }
            args(target)
        }.assertNormalExitValue()
    }
}

