class ExternalFluentMutation {
    ExternalRequest configure(ExternalRequest request, String name, int limit) {
        return request.setName(name)
                .setLimit(limit)
                .enableRetries(true);
    }
}
