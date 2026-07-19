class TryWithResourcesReturn {
    String read(Path path) {
        try (Reader reader = Files.newBufferedReader(path)) {
            return reader.readLine();
        } catch (IOException ex) {
            return "error";
        }
    }
}
