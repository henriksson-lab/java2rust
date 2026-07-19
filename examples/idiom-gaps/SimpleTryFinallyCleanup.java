class SimpleTryFinallyCleanup {
    void copy(Resource resource) throws IOException {
        try {
            resource.read();
        } finally {
            resource.close();
        }
    }
}

class Resource {
    void read() throws IOException {}
    void close() {}
}

class IOException extends Exception {}
