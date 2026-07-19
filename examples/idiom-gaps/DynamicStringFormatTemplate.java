class DynamicStringFormatTemplate {
    String describe(String label, int count) {
        String template = label + "=%d";
        return String.format(template, count);
    }
}
