class StringArrayLiteralArgument {
    void write(Writer writer, String prefix, String comment) {
        writer.writeNext(new String[]{prefix + comment}, false);
    }
}

