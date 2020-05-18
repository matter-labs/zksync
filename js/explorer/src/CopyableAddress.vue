<template>
    <code>{{address}}<i class="far fa-copy"></i></code>
</template>

<script>
export default {
    name: 'CopyableAddress',
    props: ['address'],
    data: () => ({
        hover_title: ''
    }),
    methods: {
        clicked(event) {
            this.hover_title = "Copied";
        },
        mouseEntered() {
            this.hover_title = "Click to copy"
        },
    },
};
</script>
