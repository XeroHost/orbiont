package net.xerohost.launcher.agent;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.instrument.Instrumentation;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.HashMap;
import java.util.Map;
import net.xerohost.launcher.agent.transformers.ClassTransformer;
import net.xerohost.launcher.agent.transformers.MinecraftTransformer;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassWriter;

@SuppressWarnings({"NullableProblems", "CallToPrintStackTrace"})
public final class LauncherAgent {
    private static final boolean DEBUG_AGENT = Boolean.getBoolean("launcher.debugAgent");

    public static void premain(String args, Instrumentation instrumentation) {
        final Path debugPath = Paths.get("LauncherDebugTransformed");
        if (DEBUG_AGENT) {
            System.out.println(
                    "===== Launcher agent debugging enabled. Dumping transformed classes to " + debugPath + " =====");
            if (Files.exists(debugPath)) {
                try {
                    Files.walkFileTree(debugPath, new SimpleFileVisitor<Path>() {
                        @Override
                        public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                            Files.delete(file);
                            return FileVisitResult.CONTINUE;
                        }

                        @Override
                        public FileVisitResult postVisitDirectory(Path dir, IOException exc) throws IOException {
                            Files.delete(dir);
                            return FileVisitResult.CONTINUE;
                        }
                    });
                } catch (IOException e) {
                    new UncheckedIOException("Failed to delete " + debugPath, e).printStackTrace();
                }
            }
            System.out.println("===== Quick play server version: " + QuickPlayServerVersion.CURRENT + " =====");
        }

        final Map<String, ClassTransformer> transformers = new HashMap<>();
        transformers.put("net/minecraft/client/Minecraft", new MinecraftTransformer());

        instrumentation.addTransformer((loader, className, classBeingRedefined, protectionDomain, classData) -> {
            final ClassTransformer transformer = transformers.get(className);
            if (transformer == null) {
                return null;
            }
            final ClassReader reader = new ClassReader(classData);
            final ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
            try {
                if (!transformer.transform(reader, writer)) {
                    if (DEBUG_AGENT) {
                        System.out.println("Not writing " + className + " as its transformer returned false");
                    }
                    return null;
                }
            } catch (Throwable t) {
                new IllegalStateException("Failed to transform " + className, t).printStackTrace();
                return null;
            }
            final byte[] result = writer.toByteArray();
            if (DEBUG_AGENT) {
                try {
                    final Path path = debugPath.resolve(className + ".class");
                    Files.createDirectories(path.getParent());
                    Files.write(path, result);
                    System.out.println("Dumped class to " + path.toAbsolutePath());
                } catch (IOException e) {
                    new UncheckedIOException("Failed to dump class " + className, e).printStackTrace();
                }
            }
            return result;
        });
    }
}
