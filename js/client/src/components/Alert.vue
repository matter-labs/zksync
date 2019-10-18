<template>
    <b-alert :variant="variant" fade :show="alertVisible" @dismissed="dismiss" class="mt-2" v-html="message">
    </b-alert>
</template>

<script>
export default {
    name: 'Alert',
    data: () => ({
        alertVisible: false,
        message: '',
        variant: 'info',
        timeoutHandle: null,
    }),
    methods: {
        dismiss() {
            window.clearTimeout(this.timeoutHandle);
            this.alertVisible = false;
        },
        display(kwargs) {
            this.message = kwargs.message;
            this.variant = kwargs.variant || this.variant;
            this.alertVisible = true;

            window.clearTimeout(this.timeoutHandle);
            
            if (kwargs.countdown) {
                const self = this;
                this.timeoutHandle = setTimeout(() => {
                    self.dismiss();
                }, kwargs.countdown * 1000);
            }
        }
    }
}
</script>
