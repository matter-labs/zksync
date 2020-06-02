<template>
    <code v-if="address">
        <span class="link-html-span" v-html="linkHtml" />
        <i v-if="tooltipRight===true"
            @click="clicked"
            class="far fa-copy" 
            v-b-tooltip.hover.right="hover_title"
            v-clipboard="address"
            @mouseenter="mouseEntered"
        ></i>
        <i v-else
            @click="clicked"
            class="far fa-copy" 
            v-b-tooltip="hover_title"
            v-clipboard="address"
            @mouseenter="mouseEntered"
        ></i>
    </code>
</template>

<script>
import { shortenHash } from './utils';

export default {
    name: 'CopyableAddress',
    props: ['address', 'linkHtml', 'tooltipRight'],
    data: () => ({
        hover_title: ''
    }),
    methods: {
        clicked(event) {
            this.hover_title = "Copied";
            event.stopPropagation();
        },
        mouseEntered() {
            this.hover_title = "Click to copy"
        },
    },
};
</script>

<style scoped>
.link-html-span {
    font-size: 1.0em;
}
</style>