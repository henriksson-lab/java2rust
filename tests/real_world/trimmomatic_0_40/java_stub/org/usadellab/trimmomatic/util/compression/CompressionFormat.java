package org.usadellab.trimmomatic.util.compression;

import java.io.InputStream;
import java.io.OutputStream;

/**
 * Test-only identity replacement for the optional compression dependency.
 * The differential fixture deliberately uses plain .fastq files, for which
 * the untouched Trimmomatic CompressionFormat has exactly this behaviour.
 */
public final class CompressionFormat {
    private CompressionFormat() {}

    public static InputStream wrapStreamForParsing(InputStream stream, String name) {
        return stream;
    }

    public static OutputStream wrapStreamForSerializing(
            OutputStream stream, String name, Integer compressLevel) {
        return stream;
    }
}
