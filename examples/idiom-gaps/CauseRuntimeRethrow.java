class CauseRuntimeRethrow {
    void run(Worker worker) throws Exception {
        try {
            worker.run();
        } catch (CompletionException ce) {
            throw (ce.getCause() instanceof RuntimeException ? (RuntimeException) ce.getCause() : ce);
        }
    }
}

class CompletionException extends Exception {
    Exception getCause() {
        return null;
    }
}

class RuntimeException extends Exception {
}

class Worker {
    void run() throws Exception {
    }
}
