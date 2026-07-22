class JalignerFallthroughTryCatch {
    String load(Reader reader) throws MatrixLoaderException {
        String line = null;

        try {
            line = reader.readLine();
        } catch (Exception e) {
            String message = "Failed reading from input stream: " + e.getMessage();
            throw new MatrixLoaderException(message);
        }

        return line;
    }
}

class Reader {
    String readLine() throws IOException { return "BLOSUM62"; }
}

class IOException extends Exception {}
class MatrixLoaderException extends Exception {
    MatrixLoaderException(String message) {
        super(message);
    }
}
