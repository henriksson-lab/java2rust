class TryWithResourcesCopy {
    void copy(Path source, Path dest, byte[] buffer) {
        try (InputStream input = Files.newInputStream(source);
             OutputStream output = Files.newOutputStream(dest)) {
            int n = input.read(buffer);
            output.write(buffer, 0, n);
        } catch (IOException ex) {
            throw new UserException("Could not copy file", ex);
        }
    }
}
