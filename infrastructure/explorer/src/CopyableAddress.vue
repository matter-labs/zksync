<template>
    <span v-if="address">
        <span class="link-html-span mr-1" v-html="linkHtml" />
        <i v-if="tooltipRight===true"
            @click="clicked"
            class="far fa-copy cursorpointer" 
            v-b-tooltip.hover.right="hover_title"
            v-clipboard="address"
            @mouseenter="mouseEntered"
        ></i>
        <i v-else
            @click="clicked"
            class="far fa-copy cursorpointer" 
            v-b-tooltip="hover_title"
            v-clipboard="address"
            @mouseenter="mouseEntered"
        ></i>
    </span>
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
            this.hover_title = "Click to copy";
        },
    },
};
</script>

<style scoped>
.link-html-span {
    font-size: 1.0em;
}
</style>