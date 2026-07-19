class ThrowingCallLocalResult {
    String read(boolean good) throws IOException {
        if (!good) {
            throw new IOException("bad");
        }
        return "ok";
    }

    String copy(boolean good) throws IOException {
        String value = read(good);
        return value;
    }
}

class IOException extends Exception {
    IOException(String message) {
        super(message);
    }
}
