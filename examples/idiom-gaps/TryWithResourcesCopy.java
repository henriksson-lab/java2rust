class TryWithResourcesCopy {
    void copy(Path source, Path dest, byte[] buffer) {
        try (InputStream in = Files.newInputStream(source);
             OutputStream out = Files.newOutputStream(dest)) {
            int n = in.read(buffer);
            out.write(buffer, 0, n);
        } catch (IOException ex) {
            throw new UserException("Could not copy file", ex);
        }
    }
}

