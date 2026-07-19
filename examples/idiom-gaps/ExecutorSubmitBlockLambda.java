class ExecutorSubmitBlockLambda {
    Future submit(Executor executor, Source source) {
        return executor.submit(() -> {
            String value = source.read();
            return value.trim();
        });
    }
}

class Executor {
    Future submit(Callable callable) {
        return null;
    }
}

class Callable {
}

class Future {
}

class Source {
    String read() {
        return " value ";
    }
}
