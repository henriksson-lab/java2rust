class ForEachLambdaSideEffect {
    void emitAll(List<String> names) {
        names.forEach(name -> emit(name));
    }

    void emit(String name) {
    }
}

