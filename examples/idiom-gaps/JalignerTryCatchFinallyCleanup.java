class JalignerTryCatchFinallyCleanup {
    Sequence parse(Resource resource) throws SequenceParserException {
        String sequence = null;

        try {
            sequence = resource.read();
        } catch (Exception e) {
            throw new SequenceParserException("Failed parsing sequence");
        } finally {
            try {
                resource.close();
            } catch (Exception ignored) {
            }
        }

        return new Sequence(sequence);
    }
}

class Sequence {
    Sequence(String sequence) {}
}

class Resource {
    String read() throws IOException { return "ACGT"; }
    void close() throws IOException {}
}

class IOException extends Exception {}
class SequenceParserException extends Exception {
    SequenceParserException(String message) {
        super(message);
    }
}
