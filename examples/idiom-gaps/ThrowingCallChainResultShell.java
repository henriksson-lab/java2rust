class ThrowingCallChainResultShell {
    String leaf(boolean ok) throws IOException {
        if (!ok) {
            throw new IOException("bad");
        }
        return "value";
    }

    String middle(boolean ok) {
        return leaf(ok);
    }

    String top(boolean ok) {
        String value = middle(ok);
        return value;
    }
}

class IOException extends Exception {
    IOException(String message) {
        super(message);
    }
}
