<template>
    <code v-if="address">
        <span v-html="linkHtml" />
        <i @click="clicked" 
            class="far fa-copy" 
            v-b-tooltip.hover
            :title="hover_title"
            v-clipboard="address"
            @mouseenter="mouseEntered"
        ></i>
    </code>
</template>

<script>
import { shortenHash } from './utils';

export default {
    name: 'CopyableAddress',
    props: ['address', 'linkHtml'],
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
