class MethodReferenceConstructor {
    void writeRecord(Object record) {
        DataLine line = new DataLine(1, IllegalArgumentException::new);
        consume(line);
    }

    void consume(DataLine line) {
    }
}

