plugins {
    kotlin("jvm") version "1.9.0"
    application
}

group = "com.stateset.examples"
version = "1.23.3"

repositories {
    mavenCentral()
}

dependencies {
    implementation("com.stateset:embedded-kotlin:1.23.3")
}

application {
    mainClass.set("com.stateset.examples.BasicUsageKt")
}

tasks.jar {
    manifest {
        attributes["Main-Class"] = "com.stateset.examples.BasicUsageKt"
    }
    from(configurations.runtimeClasspath.get().map { if (it.isDirectory) it else zipTree(it) })
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}
