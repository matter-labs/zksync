<template>
    <i v-b-tooltip="text" class="far fa-question-circle"></i>
</template>

<script>
export default {
    name: 'Question',
    props: ['text'],
}
</script>
